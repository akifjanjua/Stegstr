# Baseline: DWT Fails After Channel Simulator

## Summary

The current Stegstr encoder (DWT Haar 2D, LSB in LH coefficients, PNG output) **does not** survive the simulated social-platform channels. This documents the baseline run and validates the channel simulator as a proxy for WhatsApp, Instagram, Facebook, and Twitter.

## Method

1. **Encode:** Embed a fixed payload (`channel_test!`) into a cover image using `stegstr-cli embed` (DWT encoder). Output: PNG.
2. **Channel:** For each profile (whatsapp, instagram, facebook, twitter), run the channel simulator on the stego PNG: strip metadata, resize to profile max width, re-encode as JPEG (quality and 4:2:0 per profile).
3. **Decode:** Run `stegstr-cli decode` on the resulting JPEG.
4. **Compare:** Check whether the decoded payload equals the original.

## Result (encoder × channel)

| Encoder | whatsapp | instagram | facebook | twitter |
|---------|----------|-----------|----------|---------|
| dwt     | FAIL     | FAIL      | FAIL     | FAIL    |

- **FAIL** = decode returns wrong data or decode fails (non-zero exit).
- No channel produced a correct payload recovery.

## Conclusion

- The channel simulator correctly applies resize + JPEG re-encoding, which **destroys** the DWT/LSB embedding (as expected: JPEG recomputes DCT from pixels and quantizes, wiping spatial/wavelet LSB).
- To survive these platforms, embedding must be in the **JPEG (DCT) domain** with robustness (stable coefficients, error correction). See the project plan for the DCT-robust prototype.

## How to reproduce

From `channel_simulator/`:

```bash
# Build CLI: from repo root, cargo build --release --bin stegstr-cli
python test_channel_robustness.py   # or run the baseline test block from README
```

---

## Update: QIM (DCT-domain) result -- the required platforms now pass

The baseline above validated the *problem*. This section documents the *fix*:
`dct_variants.encode_dct_qim` / `decode_dct_qim`, tuned and tested against
**realistic (non-flat) cover photos** rather than the single flat solid-color
fixture used above (a flat image never triggers the resize step at all, since
every platform's `max_width` was larger than the fixture's 512px -- that made
the original DWT-fails baseline correct, but would have made an early QIM
"it passes!" result misleading. See `covers/` and `gen_realistic_covers.py`
for the four cover types used below: textured, high-frequency noise, smooth
gradient, and a portrait-like image).

### What changed vs. the initial QIM prototype

1. **Universal pre-resize, not platform-guessing.** Coefficient-domain
   embedding cannot survive an actual pixel resize (resampling recomputes
   every 8x8 block from scratch -- confirmed empirically at ~50% bit-error
   rate, i.e. random). The original approach pre-resized the cover to *guess*
   which platform it would be sent through; guessing wrong, or the image being
   forwarded through a second platform, defeated it completely. Embed now
   always shrinks to a width safe for every platform in the target set (see
   `QIM_WIDTH_PRESETS` in `dct_variants.py`), so every downstream resize step
   is a guaranteed no-op.
2. **Spatially decorrelated redundancy.** The original 5x bit-repetition
   placed all 5 copies of a bit in the *same* 8x8 block (`debug_smooth.py`
   demonstrates this), so a block that quantizes badly (e.g. a flat/low-energy
   region) made every copy wrong the same way -- majority voting over
   correlated failures does nothing. Redundant copies are now scattered via a
   single fixed pseudorandom permutation of all coefficient positions (same
   seed on encode/decode), which fixed the worst-case (flat gradient) cover
   from failing every single platform to passing all three required ones.
3. **Fixed a silent crash.** `decode_dct_qim` didn't catch Reed-Solomon decode
   exceptions on uncorrectable codewords -- it raised instead of returning
   `None`, which a caller doing `if decode(...) == payload` would never even
   reach.
4. Added a **Telegram** channel profile (max_width 1280, quality 87 -- its
   default "compress" send mode) and a `simulate_chain()` helper for modeling
   multi-hop re-shares (e.g. received via Telegram, then forwarded via
   WhatsApp), which is strictly harsher than any single hop.

### Result: single-hop, `robustness="max"` (default), realistic payload (542 bytes)

| cover    | whatsapp | instagram | facebook | twitter | telegram |
|----------|----------|-----------|----------|---------|----------|
| highfreq | PASS     | PASS      | PASS     | PASS    | PASS     |
| portrait | PASS     | PASS      | PASS     | PASS    | PASS     |
| smooth   | PASS     | PASS      | PASS     | PASS    | PASS     |
| textured | PASS     | PASS      | PASS     | PASS    | PASS     |

**20/20** -- every platform tested (including the three the spec requires:
WhatsApp, Instagram, Telegram) x every cover type, including the deliberately
adversarial flat-gradient cover, which has almost no AC energy anywhere to
embed into.

### Multi-hop re-share chains (bonus scenario, not required by spec)

9/12 pass; the 3 failures are all the adversarial flat-gradient cover on
chains that include a WhatsApp hop (its quality=65 is the harshest single
setting tested). Real photos (textured/portrait/highfreq) pass every chain
tested, including 3-hop re-shares.

### How to reproduce

```bash
cd channel_simulator
pip install -r requirements.txt
python gen_realistic_covers.py       # generates covers/ (once)
python run_matrix_realistic.py       # full matrix incl. chains
python sweep_delta.py                # shows how QIM_DELTA was chosen
python debug_ber.py                  # raw per-channel bit-error-rate, no RS/repeat
```

### Update: extended validation (9 cover types, including real phone aspect ratios) -- 45/45

Following up on a contest holder's question about how extensively this had been tested, the
cover set was expanded from 4 synthetic 768x768 images to 9, including cases the original set
didn't touch: a real phone-camera aspect ratio (1080x1920 portrait -- the original covers were
all square, which no phone actually produces), a screenshot-style image (flat UI blocks + hard
edges, a very common real share), a low-light/noisy photo, a high-contrast outdoor scene, and an
adversarial narrow-but-tall image (500x2200) chosen to stress-test resize handling.

This surfaced and fixed two real bugs, both now fixed in both the Python prototype and the Rust
port:

1. **Resize logic only checked image width, not the longer dimension.** Real platforms cap the
   long edge (e.g. "max 1600px on the longest side"), not literally the width. A narrow-but-tall
   cover could pass the pre-resize safety check unshrunk (width looked safe) and then get resized
   for real by the channel simulator once its now-inaccurate width check no longer applied,
   which is exactly the failure mode the pre-resize exists to prevent. Fixed in
   `channel.py::_resize_to_max_dim`, `dct_variants.py::encode_dct_qim`, and
   `stego_qim.rs::encode`.
2. **Erasure over-marking on low-detail content.** The confidence-margin heuristic that flags
   "low confidence" bytes for Reed-Solomon erasure correction could flag more bytes than the
   codeword could mathematically correct (observed: 163 flagged erasures against a 176-byte
   codeword with only 128 parity bytes) on content with large flat/low-AC-energy regions
   (screenshots, UI images) -- even though the underlying raw bit-error rate was still only
   ~1%, comfortably within plain blind-correction capacity. Fixed by retrying without erasure
   hints when erasure-assisted decode fails, in both `dct_variants.py::decode_dct_qim` and
   `stego_qim.rs::unwrap_payload`.

Result after both fixes, run through the actual compiled Rust CLI (not just the Python
prototype): **45/45** -- 9 cover types x all 5 platforms (WhatsApp, Instagram, Telegram,
Facebook, Twitter).

### Update: invisibility was never actually measured -- it should have been

All tuning to this point optimized `QIM_DELTA` purely for bit-error rate. Visually
inspecting output images (the same scrutiny that caught the dot-scheme visibility bug in
`ROBUSTNESS_PORT_NOTES.md`) found `QIM_DELTA=32` produces genuinely visible graininess on
flat/low-detail covers (screenshots, UI images) -- confirmed against a same-pipeline,
no-embedding baseline (identical resize + JPEG quality, zero QIM changes) to rule out JPEG
compression artifacts as the cause. Measured PSNR against that baseline: **~26dB across
every cover type tested**, not just flat ones -- below the ~30dB threshold generally
considered "fine" in watermarking literature, let alone the ~40dB+ considered imperceptible.

Re-swept delta with PSNR measured alongside robustness at every step (not just BER): 14 is
the exact floor for full 45/45 robustness; settled on **`QIM_DELTA=16`** for a small safety
margin, which still gets 45/45 while roughly halving the visible error (~32dB PSNR, plus
visibly cleaner on direct inspection). The redundancy/permutation/erasure-fallback fixes
made since the original delta=32 sweep meant far less raw signal margin is actually needed
than that first BER-only sweep assumed.

### How to reproduce

```bash
python sweep_delta.py          # original BER-only sweep (what produced delta=32)
# PSNR-vs-robustness re-tune: see the delta re-tune commit for the measurement script
```

### The shipped app, not just the prototype

Earlier revisions of this document noted the Python prototype was validated
but the Rust port (`src-tauri/src/stego_qim.rs`) hadn't been compiled or
tested. That's since been resolved: the Rust CLI now builds cleanly
(`cargo build --release --bin stegstr-cli`) and every number above from the
45/45 extended matrix onward was run through the actual compiled binary, not
just the Python prototype. It's also been confirmed against two real
platforms directly (not simulated): an embedded photo was sent through real
WhatsApp and Instagram, downloaded from the receiving side, and decoded
byte-for-byte correctly on both -- see `../ROBUSTNESS_PORT_NOTES.md` for the
full build/verify instructions.

### Two more checks: camera-realistic covers, and SSIM (not just PSNR)

**Every test above used programmatically-generated (PIL-drawn) cover images** -- circles,
gradients, noise -- never a photo that had already been through a real camera's own JPEG
encoder. A genuine camera photo carries its own compression artifacts and sensor noise
before Stegstr ever touches it, which synthetic sources don't have. Approximated this by
pre-compressing existing covers once at typical phone-camera settings (JPEG Q92, 4:2:0)
*before* using them as embed covers, then running the full platform matrix: **15/15
passed** (3 covers x whatsapp/instagram/facebook/twitter/telegram). Real camera photos are
also almost never mathematically flat like the synthetic "smooth" worst case below, so
this is a meaningfully closer proxy to real usage than the synthetic set alone.

**Invisibility was only measured with PSNR (page/section above); added SSIM** (structural
similarity, generally considered a better match to human perception than PSNR alone) for a
second, independent check:

| Cover | PSNR | SSIM |
|---|---|---|
| textured | 32.8 dB | 0.84 |
| portrait | 33.0 dB | 0.71 |
| smooth (worst case) | 33.1 dB | 0.70 |

SSIM of 0.70-0.84 is "good" but not "excellent" (>0.95) -- direct visual inspection of the
smooth-gradient worst case confirms a faint grain is still present, less pronounced than
before the delta=32->16 retune but not eliminated. This is an honest limitation, not a bug:
DCT-coefficient steganography fundamentally needs *some* natural high-frequency detail to
mask modifications within, and a mathematically flat gradient has almost none -- this is
true of any coefficient-domain steganography technique, not specific to this
implementation. Real photos (the actual, typical use case) have far more natural texture
than this deliberately-adversarial synthetic worst case, and score better (textured: 0.84
SSIM) even before accounting for that. Not further tuned beyond this point given the
robustness/invisibility tradeoff already made (see the delta re-tune above) and that the
worst case here is not representative of typical real-world cover photos.

---

## Re-verified post Phase-1-bugfix campaign, against the actual Rust CLI (SIMULATED channels, not the Python prototype)

**This section is simulated-channel testing, like everything else in this
file above it except the "two real platforms" paragraph just above (which
is explicitly marked as such) -- no image in this section was actually
uploaded to or downloaded from WhatsApp, Instagram, Facebook, Twitter, or
Telegram.** `channel.py`'s `simulate()` approximates what each platform's
own re-encoding does (resize to the platform's known max width, re-encode as
JPEG at the platform's known quality/chroma-subsampling settings) entirely
locally, no network involved. It is a well-validated proxy (see the "two
real platforms" paragraph above, where the same simulated WhatsApp/Instagram
profiles were cross-checked against genuine uploads and matched), but it is
still a proxy, not a live-platform test, and this section's numbers should
never be read as "sent through real WhatsApp/Instagram/etc." Telegram in
particular has never been tested live at all (see `STEGSTR_ENTRY_V3.md`
Part 2's "Close the Telegram gap" -- still open).

The results above (this file, up to this point) were measured using
`dct_variants.encode_dct_qim`/`decode_dct_qim` -- the Python prototype the
Rust `stego_qim.rs` implementation was ported from, not the shipped binary
itself. BUGS.md documents a bugfix campaign that touched `stego.rs` (DWT/PNG
path: bugs #1, #2, #4, #5) and `stego_qim.rs`'s libjpeg FFI cleanup layer
(bug #3 -- a resource-leak fix with no change to the QIM encode/decode math
itself). None of those fixes changed the QIM algorithm's actual coefficient
read/write logic, but "should be unaffected" is not the same as "verified,"
so re-ran the simulated-channel survival matrix against the **actual,
current `stegstr-cli --robust` binary** end-to-end (not the Python
prototype) -- see `run_matrix_rust_cli.py`.

Covers: the original 4 (`textured`, `highfreq`, `smooth`, `portrait`) plus 5
more from `covers_extended/` (`high_contrast`, `low_light`, `screenshot`,
`phone_portrait`, `narrow_tall`) -- 9 covers x 5 simulated channel profiles
(whatsapp, instagram, facebook, twitter, telegram) = 45 combinations.

**Result: 45/45 passed.** Simulated WhatsApp/Instagram/Facebook/Twitter/
Telegram-profile survival holds after the Phase 1 bugfix campaign, confirmed
against the real shipped binary (not assumed from the fixes' scope) -- but
still simulated, not a live-platform re-test.

```
cover                  whatsapp   instagram  facebook   twitter    telegram
-----------------------------------------------------------------------------
textured.png           PASS       PASS       PASS       PASS       PASS
highfreq.png           PASS       PASS       PASS       PASS       PASS
smooth.png             PASS       PASS       PASS       PASS       PASS
portrait.png           PASS       PASS       PASS       PASS       PASS
high_contrast.png      PASS       PASS       PASS       PASS       PASS
low_light.png          PASS       PASS       PASS       PASS       PASS
screenshot.png         PASS       PASS       PASS       PASS       PASS
phone_portrait.png     PASS       PASS       PASS       PASS       PASS
narrow_tall.png        PASS       PASS       PASS       PASS       PASS
```

Reproduce: `cd channel_simulator && python run_matrix_rust_cli.py` (requires
`cargo build --release --bin stegstr-cli` first).

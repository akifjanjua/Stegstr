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

### Caveat: this validates the algorithm, not (yet) the shipped app

This result is from the Python prototype in this directory, used to validate
the design before porting it into the actual Rust app. The port lives at
`src-tauri/src/stego_qim.rs` (mirrors this algorithm; wired into
`stegstr-cli embed --robust` / `decode` / `detect`) -- see
`../ROBUSTNESS_PORT_NOTES.md` for its verification status, since a Rust
toolchain wasn't available to compile-check it in the environment this port
was written in.

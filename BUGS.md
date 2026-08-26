# BUGS.md

Bugs found and fixed during the Phase 1 "break it" test campaign (see
`STEGSTR_ENTRY_V3.md`). Format per bug: repro steps, root cause, fix, commit,
regression test.

---

## Verified against pristine upstream: (a), not (b)

Cloned `brunkstr/Stegstr` (commit `ad2e10e`) into a separate directory, built
it from scratch, and ran the minimal repro for bug #1 (default embed, cover
> 256x256, payload > ~16 bytes) directly against it -- no diffing, no
assuming, an actual build and an actual run:

```
$ stegstr-cli embed photoish.png -o out.png --payload "0123456789ABCDEFGHIJ"
$ stegstr-cli decode out.png
base64:MDEyMzRTVEVHU1RSAAAAFDAxMjM=
$ echo MDEyMzRTVEVHU1RSAAAAFDAxMjM= | base64 -d
01234STEGSTR▒▒▒0123
```
Corrupted, same signature as our fork's pre-fix output. **(a) is true: this
reproduces identically on pristine upstream. It is a pre-existing bug in the
holder's app, not something this fork introduced.** A byte-for-byte diff
(`diff --strip-trailing-cr`, to rule out a line-ending false-positive)
against our pre-fix `stego.rs` (commit `2b21daa`) confirms the file is
identical to upstream's -- neither `decode()`'s tile-ordering bug (#1) nor
`embed_in_tile()`'s unguarded LSB choice (#2) had been touched before this
campaign started.

Bug #2 (clamping) and bug #5 (the tile-position pixel-corruption bug, see
below) were **also** independently confirmed present on pristine upstream
during this verification pass -- not assumed, actually reproduced by
embedding with the upstream binary and inspecting the output. See each bug's
entry for specifics.

## Backward compatibility

Concern: bug #5's fix changes embedding semantics (skipping infeasible
blocks, clamping unused ones) and bug #1/#4's fixes reorder decode
precedence. Either could break images produced by the old, unfixed encoder.

Tested by embedding with the **upstream** (unfixed) binary across 4 cover
types x 3 payload sizes (12 cases), then decoding each output with **our
fixed** binary and comparing against both the original payload and what
upstream's own decoder returned:

- **0 regressions.** Every case where upstream's own encode+decode round-trip
  already succeeded cleanly, our fixed decoder also succeeds and returns the
  identical, correct payload.
- Several cases where upstream's own decode returned WRONG bytes (pre-existing
  bug #1 corruption) are **recovered** by our fixed decoder -- it returns the
  original, correct payload from a file upstream's own tooling could never
  read correctly. This is a real, unplanned benefit: the decode-side fix
  helps old upstream-produced images too, for however long they're still
  around.
- One case (a long payload on a 1024x1024 cover) where upstream's own decode
  returned wrong bytes AND our decoder also returns wrong (but different)
  bytes -- diagnosed below (bug #2's residual case) as pre-existing embed-time
  corruption in upstream's own unfixed encoder, not recoverable by any
  decoder, and not something our fix could have caused since decode cannot
  reconstruct information a bad embed already destroyed. Re-running the exact
  same cover+payload through **our own** encoder end-to-end (not reading
  upstream's already-corrupted file) round-trips correctly.

Full backward-compat sweep output and methodology: see
`scripts/` methodology note in this file's revision history, or re-run via
the two-binary comparison (build `brunkstr/Stegstr` upstream, embed with it,
decode with this fork's binary, compare).

## Invisibility re-measurement

See bug #5 below for the full before/after PSNR/SSIM table. Short version:
the clamping fix (bug #2) itself has no measurable effect on aggregate
PSNR/SSIM (it only touches the rare individual blocks that would otherwise
clamp) -- before/after numbers for bug #2 alone are within noise of each
other across every cover type tested. The dramatic PSNR/SSIM swings actually
observed (8.96dB -> 51.16dB on a gradient; 9.02dB -> 48.64dB on random noise)
turned out to be a **separate, far more severe bug** (#5) found specifically
because this re-measurement was done at all -- see below.

## Stress coverage: hard covers, not just random noise

Random noise is the capacity-easy case for DWT (every block has plenty of AC
energy headroom); it is not where clamping bites. Re-ran the stress sweep on
the actual hard cases: flat gradients, blown-out white, near-black night
shots, uniform sky, and pure solid colors (zero AC energy anywhere, the
absolute worst case) -- 8 cover images x 12 payload lengths (1 to 2000
bytes) = 72 attempted round trips (excluding expected capacity errors), **0
failures/mismatches**. Additionally re-ran the random-noise sweep at larger
sizes (1024x1024 and 1536x1536, 10 seeds each, payloads up to 2000 bytes) to
specifically hunt for bug #2's residual "no LH value avoids clamping" case in
our own encoder: 60 round trips, **0 failures**.

## Adversarial image corpus: exact coverage manifest

Every entry actually generated and exercised (embed + decode, both encoders,
via `stegstr-cli`) in this campaign, against the specific cases named:

| requested case          | corpus file(s)                              | format/mode confirmed          |
|--------------------------|----------------------------------------------|---------------------------------|
| CMYK JPEG                 | `cmyk.jpg`                                   | JPEG, CMYK, 64x64                |
| progressive JPEG          | `progressive.jpg`                            | JPEG (progressive), RGB, 128x128 |
| EXIF-rotated              | `exif_rotated.jpg`                           | JPEG, RGB, 100x60, orientation=6 |
| palette PNG                | `palette.png`                                | PNG, mode P (indexed), 64x64     |
| 16-bit PNG                 | `sixteen_bit_gray.png`                       | PNG, mode I;16, 64x64            |
| truncated file              | `truncated.png`                              | valid PNG sig, cut at 50% length |
| corrupt file                 | `corrupt.png`                                | valid PNG sig, 50 bytes flipped  |
| non-image, image extension  | `notreally.png`, `notreally.jpg`             | plain text / fake-SOI garbage    |
| empty file                    | `empty.png`, `empty.jpg`                   | 0 bytes                          |
| wrong key                       | see bug #2/#3 entries + "Also investigated" -- app-layer "encryption" uses a fixed, non-secret app key (documented, intentional design, not a per-recipient secret); tested tampered-ciphertext (bit-flipped) instead, correctly rejected by AES-GCM's auth tag |
| double-embedded                  | re-embedded a second, shorter payload into an already-embedded output, both encoders -- decodes the second payload correctly, both DWT and QIM |
| zero-byte payload                 | `--payload-base64 ""` -- round-trips correctly, both encoders |
| payload exactly at capacity        | binary-searched the exact boundary on both encoders -- capacity succeeds, capacity+1 fails cleanly with a descriptive error, no crash |
| grayscale PNG                       | `grayscale.png`                              | PNG, mode L, 64x64                |
| PNG with alpha                       | `with_alpha.png`                             | PNG, mode RGBA, 64x64             |
| interlaced PNG                        | `interlaced.png`                             | PNG (Adam7 interlace), RGB, 64x64 |
| animated PNG (APNG)                    | `animated.png`                               | PNG, 3 frames, RGB, 32x32         |
| 1x1 / 1x2 / 2x1 / 2x2 / 3x3 images      | `tiny_*.png`                                 | PNG, RGB, exact sizes named        |
| 7x7 / 8x8 / 9x9 / 16x16 images            | `small_*.png`                                | PNG, RGB, exact sizes named         |
| non-square, odd dims                       | `odd_15x17.png`                              | PNG, RGB, 15x17                     |
| large multi-tile covers                     | `large_1024.png` (1024x1024), `large_2048.jpg` (1500x900) | PNG/JPEG, RGB |
| flat/featureless covers                      | `flat_gray.png`/`.jpg`, plus `hard_covers/`: `flat_gradient`, `blown_out_white`, `near_black_night`, `uniform_sky`, `pure_solid_gray/white/black` | see bug #5 table |
| paths with spaces / non-ASCII                 | tested directly (`weird path with spaces/日本語ディレクトリ/cöver ímage.png`) -- embed + decode both succeed |
| very long Windows paths                        | tested directly (363-char path, well past classic `MAX_PATH`=260) -- succeeds |
| read-only output directory/file                 | tested directly -- clean `Access is denied (os error 5)`, no crash |
| two invocations racing on the same file           | tested directly (10 concurrent `embed` calls to the same output path) -- no crash, no corrupted file, last-writer-wins, clean decode |
| emoji / RTL text / null bytes payloads             | included in the `attack.py` payload battery run against every corpus image, both encoders -- no crashes |
| multi-megabyte payloads                             | 1MB and 5MB payloads tested against large-capacity covers -- clean capacity errors or successful embeds, no crash |

**Payload edge cases battery** (run against every corpus image above, both
encoders): empty, 1 byte, short, emoji+accented text, RTL (Arabic) text,
embedded null bytes, 1KB, 10KB, 1MB, 5MB.

---

## 1. DWT decode returns silently corrupted payload for any image >= 256px with a payload over ~9 bytes

**Severity:** Critical. This is the default encoder (`stegstr-cli embed` without
`--robust`), so this affected essentially every real use of it.

**Repro (pre-fix):**
```bash
stegstr-cli embed cover_512x512.png -o out.png --payload-base64 <(echo -n "0123456789" | base64)
stegstr-cli decode out.png
# => base64:AAAAASTEGS...   (garbage, not the original 10 bytes)
```
Any payload of 10+ raw bytes (i.e. `to_embed = MAGIC(7) + LEN(4) + payload`
exceeding 128 bits) on a cover >= 256px in both dimensions reproduced this.
A 5-byte payload happened to round-trip correctly, which is why the existing
unit test (a 256x256 single-tile image) never caught it.

**Root cause:** `stego::encode()` embeds independently into each 256x256 tile
using a Haar2D transform local to that tile (256-wide block grid). `decode()`
tried a **whole-image** Haar2D transform first ("backward compat"), which
indexes LH coefficients by the image's *full* width, not the tile's local
256-wide grid. The two orderings only agree for the first 128 bits (one
tile-local row); beyond that the whole-image bit sequence silently mixes in
data from the *adjacent* tile. `decode_from_tile`'s magic search has no
integrity check beyond "declared length fits available bits", so it returned
a magic-shaped but corrupted match instead of ever reaching the (correct)
tile-aligned sliding-window search that came after it in the old code.

**Fix:** Reordered `decode()` to try the tile-aligned sliding-window search
first (this reproduces the encoder's exact per-tile transform for any
un-cropped image), falling back to the whole-image interpretation only for
images smaller than one tile or the rare case where `encode()` itself fell
back to a whole-image transform (no single tile had capacity).

**Commit:** `699f4cb`

**Regression test:** `stego::tests::test_encode_decode_roundtrip_multi_tile_long_payload`
(512x512 cover, 100-byte payload) in `src-tauri/src/stego.rs`.

---

## 2. DWT decode flips bits on high-contrast/noisy covers (silent clamping in the Haar inverse transform)

**Severity:** High. Any cover with pixels at or near 0/255 (camera noise/grain,
screenshots, synthetic/high-contrast images, some CMYK-JPEG-derived RGB) could
corrupt scattered bytes of the payload.

**Repro (pre-fix, after bug #1's fix alone):**
```bash
# a cover with independent per-channel random noise
stegstr-cli embed noise_256x256.png -o out.png --payload "The quick brown fox jumps over the lazy dog. (x5)"
stegstr-cli decode out.png
# => single scattered byte flips, e.g. "brown fox" -> "Brown fox"
```

**Root cause (two-part):**
- `embed_in_tile()` picked the LH coefficient value using only LSB parity
  (`(lh & !1) | bit`), regardless of whether the pixels it reconstructs to
  stay within `[0, 255]`. A pixel already within 1 of a boundary (common with
  noisy/high-contrast data) pushed the reconstruction out of range;
  `haar2d_inverse` silently `.clamp(0, 255)`s it, and decode's forward
  transform then reads back the flipped LSB. (Proved algebraically: the /4
  truncation in the forward transform divides evenly back out of the inverse
  formulas as long as no clamping occurs, so avoiding clamping makes the
  round trip exact, not just "usually fine".)
- Even after fixing the above (`pick_safe_lh`, searching nearby LH values
  with the correct parity for one that avoids clamping), a residual case
  remained: some blocks have **only one** LH value in existence that avoids
  clamping, and it may have the wrong parity for the bit that needs encoding
  there. Skipping such blocks (`both_bits_feasible`) fixed the direct case,
  but *leaving them untouched* could still let their own natural LH value
  clamp, which drifts that block's recomputed `(ll, hl, hh)` on decode and
  can flip its `both_bits_feasible` classification -- desyncing which blocks
  encoder and decoder each think carry data, corrupting everything from that
  block onward. (Caught via `cmyk.jpg` in the adversarial corpus: instrumented
  encode/decode to print their skip decisions and diffed them -- they
  disagreed on 6 of ~46 blocks.)

**Fix:**
- `pick_safe_lh()`: choose the LH value closest to natural (of the required
  parity) that reconstructs all four pixels without clamping.
- `both_bits_feasible()` / `safe_lh_for_unused_block()`: a block only carries
  a payload bit if *both* possible bit values are representable there without
  clamping (computed from `(ll, hl, hh)` alone, so both sides always agree);
  every other block -- used for payload or not -- gets nudged to a clamp-free
  LH value too, so no block's `(ll, hl, hh)` can ever drift between what
  encoder and decoder each compute.

**Residual limitation (documented, not fixed):** a block where *no* LH value
at all avoids clamping (`lh_min > lh_max` in `lh_bounds()`) is mathematically
unavoidable to distort. This is strictly rarer than the fixed case above.
**Update after upstream verification:** decoding a file *upstream's own
(unfixed) encoder* wrote for a 1024x1024 random-noise cover with a 141-byte
payload showed exactly this residual single-byte-flip signature -- so the
underlying mathematical edge case is real and does occur on sufficiently
large/adversarial covers, not just a theoretical corner case. However, that
corruption happened during upstream's *embed* (its unfixed
`(lh & !1) | bit` has no clamp-avoidance at all), which no decoder can
retroactively repair -- it is not evidence our fix has a gap. Re-running our
own encoder+decoder end-to-end on that exact cover+payload (and a further
60-case sweep at 1024x1024 and 1536x1536 with random seeds and payloads up to
2000 bytes) found 0 mismatches. The residual case remains real in principle
but still unobserved from *our* encoder specifically. If it matters for full
closure, the general fix is the same idea Phase 4 already plans for the QIM
path: skip such blocks from capacity entirely and let tile-level redundancy
cover the loss.

**Commit:** `699f4cb`

**Regression test:** `stego::tests::test_encode_decode_roundtrip_high_contrast_cover`
(deterministic xorshift-random 256x256 cover) in `src-tauri/src/stego.rs`.
Also verified with a 252-case stress sweep (7 sizes x 6 seeds x 6 payload
lengths, uniform random noise covers) and the full adversarial image corpus
(grayscale, palette, 16-bit, interlaced, animated, CMYK, progressive,
EXIF-rotated) -- 0 mismatches, down from 7 before this fix.

---

## 3. Malformed-JPEG decode leaks a file handle + libjpeg memory pool on every attempt

**Severity:** Medium. Not a crash (already caught by `catch_unwind`), but a
real, repeatable resource leak on a routine input path.

**Repro:**
```bash
printf '\xff\xd8not really jpeg data after SOI marker' > fake.jpg
stegstr-cli decode fake.jpg
# stderr: "thread 'main' panicked at ...: libjpeg error: JPEG datastream contains no image"
# exits 1 (not a crash) -- but see root cause: the FILE* and libjpeg's
# internal decompress memory pool for this attempt are never freed.
```
`decode_any()` (lib.rs) tries the QIM/JPEG decoder against **every** image
passed to it regardless of extension, so this fires on every malformed-JPEG
decode attempt, not a rare corner case -- any truncated or corrupt JPEG whose
first two bytes still happen to be a valid SOI marker (routine: any real
photo cut short mid-transfer) hits it.

**Root cause:** `ffi::error_exit` (the libjpeg error handler) calls `panic!()`
so routine libjpeg errors (corrupt/truncated input) can be turned into a
normal `Result::Err` via `catch_unwind` at the `read_y_coefficients` /
`write_y_coefficients` boundary. But `jpeg_destroy_decompress`/`_compress` and
`fclose` were only called on the non-panicking path -- a panic mid-read/write
unwound straight past them.

**Fix:** Added `DecompressGuard` / `CompressGuard` RAII wrappers around each
libjpeg session + its `FILE*`, so cleanup runs in `Drop` -- guaranteed on both
the normal-return and panic-unwind paths.

**Commit:** `699f4cb`

**Regression test:** none added (the leak itself isn't practically observable
from a single-process unit test on Windows' default handle limits without an
OS-specific handle-counting harness). Verified by re-running the fixed binary
against `notreally.jpg` (fake SOI + garbage body) 20x in a row and confirming
identical, clean `Err` behavior each time (no behavior change expected or
observed -- the fix is only visible via not leaking, which the RAII guarantee
makes structurally true rather than something to assert at runtime).

---

## 4. `decode()`'s tile-aligned search silently skipped for "one axis large, other small" images

**Severity:** High. Same corruption class as bug #1, for a geometry #1's fix
didn't cover.

**Found by:** a proptest property test added specifically to fuzz this class
of bug (`prop_roundtrip_random_cover_and_payload`, random cover dimensions
8..320, random payload 0..600 bytes) -- found a failing case within the
default 40 generated cases, on the *first run after adding the test*.
Minimized automatically to `w = 258, h = 8`.

**Repro:**
```bash
# a cover wider than one tile but shorter than one tile
stegstr-cli embed cover_258x8.png -o out.png --payload "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ0123"
stegstr-cli decode out.png
# => corrupted output, not the original payload
```

**Root cause:** `encode()` tiles each axis independently -- its loop steps by
`TILE_SIZE` on x and y separately -- so a "wide but short" or "narrow but
tall" image (only ONE dimension >= `TILE_SIZE`) still gets tiled along that
one axis. A 258x8 image becomes a 256x8 tile at x=0 plus a near-empty 2x8
remainder (too small to hold anything, skipped). But `decode()`'s tile-aligned
search only ran when **both** `w >= TILE_SIZE` and `h >= TILE_SIZE`
(`&&`), so for this shape it skipped the tile-aligned search entirely and
fell straight to the whole-image interpretation -- the same wrong-bit-ordering
failure as bug #1, just for a shape the `&&` guard exempted.

**Fix:** Changed the guard from `&&` to `||`. The existing per-axis
`saturating_sub`/`step_by` math already handles the smaller axis correctly
(it collapses to a single `0` offset) once the loop is allowed to run at all.

**Commit:** `e65d722`

**Regression test:** `stego::tests::prop_roundtrip_random_cover_and_payload`
(the proptest itself -- its regression corpus at
`src-tauri/proptest-regressions/stego.txt` pins this exact minimized case so
it's replayed on every future run before any new cases are generated).
Verified with a follow-up 500-case run (`PROPTEST_CASES=500`), plus the full
corpus and 252-case stress sweep, all still 0 mismatches.

---

## 5. `encode()` wrote every non-leftmost tile's pixels from the WRONG source column -- catastrophic visible corruption, not just imperfect invisibility

**Severity:** Critical, and arguably the most user-visible bug in this
campaign. It doesn't corrupt the payload (decode still recovers it correctly
from the untouched leftmost tile), but it visibly wrecks the cover image
itself for any image wider than one 256px tile -- which defeats the entire
point of steganography (the image is supposed to look unmodified).

**Found by:** re-measuring PSNR/SSIM invisibility (before/after the clamping
fix, bug #2) on a set of "hard" covers -- flat gradients, blown-out white,
near-black, uniform sky, plain random noise -- per an explicit request to
verify the clamping fix's actual visual impact. A 512x512 gradient scored
PSNR 8.96dB / SSIM 0.75, and a 1024x1024 random-noise cover scored PSNR
9.02dB / SSIM 0.25. Every other/smaller cover scored 30-67dB -- these two were
wildly, suspiciously worse, which is what prompted digging into WHY rather
than just reporting the number.

**Repro:**
```bash
# any cover wider than 256px, e.g. a 512x512 image
stegstr-cli embed cover_512x512.png -o out.png --payload "short message"
# out.png: columns 0-255 (the leftmost tile) look normal.
# columns 256-511 are visibly a different image entirely -- literally a
# copy of columns 0-255's original content, not the cover's actual content
# there. Payload still decodes fine (bug is invisible to any
# payload-correctness check), but the image is grossly, visibly altered.
```

**Root cause:** `encode()`'s tile-extraction loop computed each tile's row
read offset as `(ty + y) * w * 4`, omitting `+ tx * 4`. Every tile except the
leftmost column (`tx = 0`) therefore read pixel data starting at column 0
instead of its own x position, ran that (wrong) data through
`embed_in_tile`, and wrote the result back at its own (correct) output
position via `out_row_start = (ty + y) * w * 4 + tx * 4` (which DID have the
offset) -- silently overwriting that entire tile region with a copy of the
leftmost tile's content. The decode-side equivalent read
(`decode()`'s sliding-window loop, further down the same file) already had
the correct `+ ox * 4` term; only this one encode-side read was missing it.

This was invisible to every payload-correctness round-trip test run in this
entire campaign -- the 252-case stress sweep, the full adversarial corpus,
the proptest, the hard-cover sweep -- because `decode()` finds the payload in
the untouched `tx = 0` tile and returns immediately. None of those tests ever
compared the cover to the stego output pixel-for-pixel; they only checked
"does the payload come back correctly," which structurally cannot see this
bug. It only surfaced once asked to measure invisibility, not just
correctness.

**Confirmed present in pristine upstream** (brunkstr/Stegstr, commit
`ad2e10e`) -- this exact code, never touched by any of this fork's other
fixes. Not fork-introduced.

**Fix:** Added the missing `+ tx * 4`.

**Commit:** `08bd234`

**Before/after PSNR / SSIM** (payload: "The quick brown fox..." x4, 188
bytes; see `channel_simulator/` methodology notes for measurement approach --
here computed with scikit-image's reference PSNR/SSIM implementations):

| cover                        | PSNR before | SSIM before | PSNR after | SSIM after |
|-------------------------------|------------:|------------:|-----------:|-----------:|
| flat_gradient (512x512)       |    8.96 dB  |      0.7521 |  51.16 dB  |     0.9956 |
| blown_out_white (512x512)     |   48.59 dB  |      0.9873 |  49.96 dB  |     0.9912 |
| near_black_night (512x512)    |   43.50 dB  |      0.9454 |  49.02 dB  |     0.9807 |
| uniform_sky (512x512)         |   44.89 dB  |      0.9684 |  49.19 dB  |     0.9895 |
| pure_solid_gray (512x512)     |   66.65 dB  |      0.9998 |  66.65 dB  |     0.9998 |
| photoish, textured (512x512)  |   31.34 dB  |      0.8430 |  50.41 dB  |     0.9944 |
| random_noise (1024x1024)      |    9.02 dB  |      0.2527 |  48.64 dB  |     0.9999 |

`pure_solid_gray` is unchanged (66.65dB both before and after) -- expected:
a uniform-color image has identical content in every tile regardless of
which tile's data actually gets read, so that cover can't reveal this bug
either way. That row is a useful internal-consistency check on the
measurement itself, not evidence the fix did nothing.

**Regression test:** `stego::tests::test_encode_does_not_corrupt_non_leftmost_tiles`,
which asserts image fidelity (small LSB-scale pixel deltas, not payload
recovery) in a non-leftmost tile -- specifically to catch what
payload-only round-trip tests structurally cannot.

---

## Also investigated, not a bug

- **`--payload-base64` has no `@file` form.** `--payload @file` requires the
  file to be valid UTF-8 (it's read via `fs::read_to_string`); embedding an
  arbitrary binary file requires `--payload-base64 <b64>`, which only accepts
  an inline string, not a file -- and inline base64 for anything past roughly
  tens of KB exceeds Windows' ~32K command-line length limit. Not fixed (CLI
  UX gap, not a correctness bug); flagged for Phase 5 (`scripts/`) or a future
  `--payload-base64 @file` option.
- **`stego_crypto`'s "encrypt" is app-wide obfuscation, not per-recipient
  secrecy.** The AES key is derived from a fixed, public salt baked into the
  source (`APP_KEY_SALT`), so any Stegstr build can decrypt any
  `--encrypt`-ed payload -- this is documented/intentional ("any Stegstr user
  can detect"), not a "wrong decryption key" bug. Tampered-ciphertext input
  (bit-flipped) is correctly rejected by AES-GCM's auth tag, verified.
- **Capacity boundary (exactly-at-capacity / one-byte-over) for both encoders**
  is off-by-one-clean: verified via binary search on both DWT and QIM paths --
  exact-capacity succeeds, capacity+1 fails with a descriptive error, no crash.

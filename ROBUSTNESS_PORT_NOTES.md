# Robustness work: what changed, what's verified

This documents the contest-driven work in this fork: making Stegstr survive
being sent through WhatsApp, Instagram, and Telegram (the spec's core
requirement -- the reference implementation's own docs admit it doesn't:
`wiki/how-it-works.html` says "Avoid JPEG or other lossy formats -- recompression
can destroy the hidden data").

## Summary

- A DCT-domain QIM (Quantization Index Modulation) encoder replaces the
  original spatial-domain DWT embedding, which is destroyed by any platform's
  JPEG re-compression.
- **Ported to Rust and compiled successfully** (`src-tauri/src/stego_qim.rs`),
  wired into the CLI as `stegstr-cli embed --robust` / `--robustness
  standard|max`, and into `decode`/`detect` (which try the QIM/JPEG decoder
  first, falling back to the original PNG/DWT decoder, since a recipient
  doesn't know which encoder produced an image they were sent).
- **Result: 45/45** -- 9 realistic cover-image types (including a real
  phone-camera aspect ratio, a screenshot/UI-style image, low-light, high-
  contrast, and an adversarial narrow-tall case) x all 5 platforms tested
  (WhatsApp, Instagram, Telegram, Facebook, Twitter), run through the actual
  compiled Rust binary. Full methodology and numbers:
  `channel_simulator/BASELINE_RESULTS.md`.
- **Confirmed on real platforms, not just simulation:** an embedded photo was
  sent through real WhatsApp and Instagram, downloaded from the receiving
  side, and decoded byte-for-byte correctly on both. Instagram genuinely
  re-encoded the file (file size changed), confirming it was a real
  recompression test.

## How to build and verify yourself

```bash
cd src-tauri
cargo build --release --bin stegstr-cli
```

Windows note: this needs a C compiler in addition to Rust (mozjpeg-sys
compiles a C library from source) -- Visual Studio Build Tools with the
"Desktop development with C++" workload, or `winget install
Microsoft.VisualStudio.2022.BuildTools`.

```bash
# Round-trip sanity check
./target/release/stegstr-cli embed cover.jpg -o out.jpg --robust --payload "hello world"
./target/release/stegstr-cli decode out.jpg
# should print: hello world

# Real-world validation (this is also literally the contest's own test method):
# send out.jpg through WhatsApp / Telegram / Instagram to another device,
# save the received image, then:
./target/release/stegstr-cli decode received.jpg
```

The Python prototype (`channel_simulator/dct_variants.py`) is also a
complete, independently-testable implementation of the same algorithm, useful
for quickly trying new cover images or channel conditions without a Rust
rebuild -- see `channel_simulator/README.md`.

## Two real bugs found by extending the test coverage

The initial validation used 4 synthetic 768x768 (square) cover images. After
a contest holder asked how extensively this had been tested, the cover set
was expanded to 9 types, including a real phone-camera aspect ratio and a
screenshot-style image -- neither of which the original set exercised. This
found two real, since-fixed bugs (see `channel_simulator/BASELINE_RESULTS.md`
for the full writeup):

1. The pre-resize safety check only looked at image *width*; real platforms
   constrain by the *longer* side. A narrow-but-tall cover could pass
   unshrunk and then get resized for real downstream.
2. The Reed-Solomon "erasure" confidence heuristic over-triggered on flat/
   low-detail content (screenshots), flagging more bytes as unrecoverable
   than the codeword could mathematically correct, even though the
   underlying signal was still fine. Fixed by falling back to plain blind
   error correction when erasure-assisted decode fails.

Both are fixed in both the Python prototype and the Rust port. Worth noting
for anyone extending this further: this class of bug (something that only
shows up on content types the original test set didn't include) is exactly
why testing against varied, realistic images matters more than testing
against many *quality settings* of the same few images.

## Files touched

| File | What |
|---|---|
| `channel_simulator/dct_variants.py` | QIM encode/decode: universal safe-width (longer-side) pre-resize, spatially-decorrelated permutation-based redundancy, RS exception fix, erasure-fallback fix. |
| `channel_simulator/channel.py` | Added `telegram` profile, `simulate_chain()` for multi-hop re-share testing, longer-side resize fix. |
| `channel_simulator/gen_realistic_covers.py`, `gen_extended_covers.py` | Generate realistic (non-flat, varied-aspect-ratio) test covers -- the original fixture was a flat 512x512 color, which never exercised the resize path at all. |
| `channel_simulator/run_matrix_realistic.py`, `sweep_delta.py`, `debug_ber.py`, `debug_smooth.py` | The test/tuning harness behind the numbers in `BASELINE_RESULTS.md`. |
| `src-tauri/src/stego_qim.rs` | Rust port: QIM embed/decode, JPEG DCT coefficient FFI (mozjpeg-sys), Reed-Solomon chunking, fixed permutation. Compiles and passes the full 45/45 matrix. |
| `src-tauri/src/lib.rs` | Registered the module; added `decode_any()` trying both encoders. |
| `src-tauri/src/bin/stegstr_cli.rs` | `embed --robust` / `--robustness`; `decode`/`detect` use `decode_any`. |
| `src-tauri/Cargo.toml` | Added `mozjpeg-sys`, `reed-solomon`. |
| `src/relay.ts` | Reconnection with exponential backoff on drop (previously: a dropped connection just stayed dead); `publish()` now resolves with a confirmed-relay count instead of firing blind, so callers can tell a post/DM actually reached a relay. |
| `src/App.tsx` | Surfaces publish failures via the existing toast system instead of failing silently; shows "Reconnecting…" relay status. |

## What wasn't attempted

Full cross-platform release builds (macOS/Linux/.deb/.AppImage/APK) weren't
produced -- built and tested on Windows only. `cargo build --release --bin
stegstr-cli` on macOS/Linux should work unchanged (no Windows-specific code
in the new module), but hasn't been confirmed there.

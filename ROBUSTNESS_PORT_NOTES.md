# Robustness work: what changed, what's verified, what to check next

This documents the contest-driven work in this fork: making Stegstr survive
being sent through WhatsApp, Instagram, and Telegram (the spec's core
requirement -- the reference implementation's own docs admit it doesn't:
`wiki/how-it-works.html` says "Avoid JPEG or other lossy formats -- recompression
can destroy the hidden data").

## Summary

- **Validated in Python** (`channel_simulator/`): a DCT-domain QIM
  (Quantization Index Modulation) encoder, tuned against realistic (non-flat)
  cover photos, passes **20/20** -- every platform tested (WhatsApp,
  Instagram, Telegram, Facebook, Twitter) x every cover type tested (textured,
  high-frequency, smooth gradient, portrait-like), including the deliberately
  adversarial flat-gradient case. Full methodology and numbers:
  `channel_simulator/BASELINE_RESULTS.md`.
- **Ported to Rust** (`src-tauri/src/stego_qim.rs`): mirrors that algorithm,
  wired into the CLI as `stegstr-cli embed --robust` / `--robustness
  standard|max`, and into `decode`/`detect` (which now try the QIM/JPEG
  decoder first, falling back to the original PNG/DWT decoder -- a recipient
  doesn't know which encoder produced an image they were sent, so this needs
  to be transparent).

## Verification status -- please run this before relying on the Rust path

The Python algorithm is fully tested (see above). The Rust port's pure logic
(QIM math, bit packing, the fixed permutation, Reed-Solomon chunking) is
plain deterministic Rust with no external unknowns. The one part written
against fetched documentation rather than a local compiler is the `ffi`
submodule inside `stego_qim.rs`: libjpeg DCT coefficient read/write via
`mozjpeg-sys`, following the standard `jpegtran.c` coefficient-copy pattern
(`jpeg_read_coefficients` -> mutate the Y-plane in place via
`access_virt_barray` -> `jpeg_copy_critical_parameters` ->
`jpeg_write_coefficients`, which passes Cb/Cr and quantization tables through
byte-for-byte unchanged). No Rust toolchain was available in the environment
this was written in, so **this specific module has not been compiled**.

Please run, in order:

```bash
cd src-tauri
cargo build --release --bin stegstr-cli
```

**If it doesn't compile:** the error will almost certainly be inside
`stego_qim::ffi` (a small, isolated module -- everything else in the file is
plain Rust) and almost certainly a small signature mismatch against whatever
exact `mozjpeg-sys` version Cargo resolves (struct field name, `boolean` vs
`bool`, an `Option<unsafe extern "C-unwind" fn(...)>` shape). Paste the error
back and it's a quick fix -- the algorithm design isn't in question, only
whether these specific FFI declarations match the resolved crate version
exactly.

**Windows note:** `mozjpeg-sys` compiles the mozjpeg C library from source and
needs a working C toolchain (MSVC Build Tools, or the Visual Studio "Desktop
development with C++" workload) in addition to Rust itself.

Once it compiles, verify the actual claim -- that the robust path round-trips
and (unlike the original DWT path) survives recompression:

```bash
# Round-trip sanity check
./target/release/stegstr-cli embed cover.png -o out.jpg --robust --payload "hello world"
./target/release/stegstr-cli decode out.jpg
# should print: hello world

# Real-world validation (this is also literally the contest's own test method):
# send out.jpg through WhatsApp / Telegram / Instagram to another device,
# save the received image, then:
./target/release/stegstr-cli decode received.jpg
```

If the Rust build has issues that can't be resolved in time, the **Python
prototype is a complete, working, independently-testable implementation of
the same algorithm** (`channel_simulator/dct_variants.py`,
`qim_cli.py`) -- see `channel_simulator/README.md` for its own CLI.

## Files touched

| File | What |
|---|---|
| `channel_simulator/dct_variants.py` | QIM encode/decode: universal safe-width pre-resize, spatially-decorrelated permutation-based redundancy, RS exception fix. |
| `channel_simulator/channel.py` | Added `telegram` profile, `simulate_chain()` for multi-hop re-share testing. |
| `channel_simulator/gen_realistic_covers.py` | New: generates non-flat test covers (the original fixture was a flat 512x512 color, which never exercises the resize path at all). |
| `channel_simulator/run_matrix_realistic.py`, `sweep_delta.py`, `debug_ber.py`, `debug_smooth.py` | New: the test/tuning harness behind the numbers in `BASELINE_RESULTS.md`. |
| `src-tauri/src/stego_qim.rs` | New: Rust port, see verification status above. |
| `src-tauri/src/lib.rs` | Registered the module; added `decode_any()` trying both encoders. |
| `src-tauri/src/bin/stegstr_cli.rs` | `embed --robust` / `--robustness`; `decode`/`detect` use `decode_any`. |
| `src-tauri/Cargo.toml` | Added `mozjpeg-sys`, `reed-solomon`. |
| `src/relay.ts` | Reconnection with exponential backoff on drop (previously: a dropped connection just stayed dead); `publish()` now resolves with a confirmed-relay count instead of firing blind, so callers can tell a post/DM actually reached a relay. |
| `src/App.tsx` | Surfaces publish failures via the existing toast system instead of failing silently; shows "Reconnecting…" relay status. |

## What wasn't attempted

Full cross-platform release builds (macOS/Windows/Linux/.deb/.AppImage/APK)
weren't produced or tested -- that needs the actual target OSes/signing setup
this environment doesn't have. `cargo build --release --bin stegstr-cli` on
whatever platform you're testing from is the fastest path to confirming the
core claim.

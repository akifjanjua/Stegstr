# Stegstr

<!-- Demo video slot: STEGSTR_VIDEO_ENTRY_BRIEF.md's shot list, once recorded. -->

**Steganographic social networking.** Hide messages in images and share them anywhere—local-first, with optional Nostr sync.

**This fork adds JPEG-domain (QIM) steganography that survives WhatsApp and Instagram recompression** (the original PNG/DWT embedding does not -- upstream's own docs say to avoid JPEG). See [`ROBUSTNESS_REPORT.md`](ROBUSTNESS_REPORT.md) for the full before/after numbers, live-platform confirmation, and an honest "what we did not test" section, or [`BUGS.md`](BUGS.md) for the 8 bugs found and fixed along the way — 5 pre-existing in the upstream application (each verified against a pristine clone of `brunkstr/Stegstr`), 3 in this fork's own new work, one security issue in each.

Stegstr gives you two ways to use it:

- **UI app** — Desktop and mobile app. Create posts, embed them in images, and detect content from images with a graphical interface.
- **CLI module** — Command-line tool for scripts and automation. Decode, detect, embed, and create Nostr-style posts from the terminal.

Both use the same steganographic format. Data is stored and processed **locally**; Stegstr is **not exclusively Nostr**. You can use it fully offline (embed/detect in images and share via any channel). When you want to sync over the network, Stegstr can act as a Nostr client and use relays.

## Quick start

### Graphical app (UI)

This fork doesn't have pre-built release binaries yet (no CI release pipeline set
up here) — build from source instead, see "Build from source (full app)" below.
(The links a reader might expect here for pre-built downloads would point at the
upstream project's releases, which do **not** include the fixes in this fork --
see [`ROBUSTNESS_PORT_NOTES.md`](ROBUSTNESS_PORT_NOTES.md) for what changed and why
building from this fork specifically matters.)

### Command-line interface (CLI)

You need [Rust](https://rustup.rs) (latest stable). Clone and build the CLI:

```bash
git clone https://github.com/akifjanjua/Stegstr.git
cd Stegstr
cd src-tauri && cargo build --release --bin stegstr-cli
```

Binary: `target/release/stegstr-cli` (Windows: `stegstr-cli.exe`). Example:

```bash
./target/release/stegstr-cli post "Hello from CLI" --output bundle.json
./target/release/stegstr-cli embed cover.png -o out.png --payload @bundle.json --encrypt
./target/release/stegstr-cli detect out.png
```

**Sending through WhatsApp, Instagram, or Telegram?** Add `--robust` -- it
switches to the JPEG/DCT (QIM) encoder, which is built to survive those
platforms' own re-compression (the example above, without `--robust`, does
not survive being re-uploaded). Output is always a `.jpg`:

```bash
./target/release/stegstr-cli embed cover.jpg -o out.jpg --robust --payload "hello world"
# send out.jpg through WhatsApp/Instagram/Telegram, download the received copy, then:
./target/release/stegstr-cli decode received.jpg
# decode tries the robust JPEG/QIM decoder first, then falls back to PNG/DWT --
# you don't need to know which encoder produced an image you were sent.
```

See [`ROBUSTNESS_REPORT.md`](ROBUSTNESS_REPORT.md) for what "survives" is
actually backed by (live sends, not just simulation, with the mechanism
behind each result spelled out).

## Verify it yourself

```bash
./scripts/verify.sh     # clean clone -> Rust build/test/clippy -> npm test -> full channel matrix
./scripts/demo.sh        # fixed cover, fixed payload: embed + decode round trip
```

Windows: `scripts\verify.ps1` (same steps, PowerShell). Needs Rust, and
optionally Node.js + Python for the frontend tests and channel-simulator
matrix (skippable: `./scripts/verify.sh --skip-python`).

**Expect this to take a while on a cold machine** — roughly 10-20 minutes
with no warm caches (network- and CPU-dependent), most of it the Rust
release build alone (measured at ~7-8 minutes from an empty `target/`).
The script prints a heartbeat line every 20 seconds during long steps and
a `[step N]` marker with elapsed time per step, so a quiet stretch is
normal, not a hang — `cargo`'s own `Compiling <crate>` output is the sign
of life during the build itself.

If `npm test` reports a timeout waiting for a worker process to start,
that's a resource-contention flake (something else on the machine was
compiling at the same moment), not a real test failure — it doesn't
happen when nothing else is running concurrently. Just re-run it.

## Build from source (full app)

Prerequisites: Node.js 18+, Rust (latest stable).

```bash
git clone https://github.com/akifjanjua/Stegstr.git
cd Stegstr
npm install
npm run build:mac   # or build:win, build:linux
```

See the repo for platform-specific build deps (e.g. Xcode CLI tools, Visual Studio Build Tools, Linux dev packages).

## Links

- [Website](https://stegstr.com) — Downloads, getting started, wiki (this is the original upstream project's site, not this fork's)
- [Wiki / CLI docs](https://stegstr.com/wiki/cli.html) — Full CLI reference
- [This fork's source](https://github.com/akifjanjua/Stegstr)
- [Robustness report](ROBUSTNESS_REPORT.md) — before/after numbers, live-platform confirmation, what wasn't tested
- [Bugs found and fixed](BUGS.md) — 8 bugs (5 pre-existing upstream, 3 in this fork's own work), repro steps, root cause, fix, regression test each
- [What changed in this fork and why](ROBUSTNESS_PORT_NOTES.md)

## License

MIT

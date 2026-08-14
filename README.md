# Stegstr

**Steganographic social networking.** Hide messages in images and share them anywhere—local-first, with optional Nostr sync.

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
- [What changed in this fork and why](ROBUSTNESS_PORT_NOTES.md)

## License

MIT

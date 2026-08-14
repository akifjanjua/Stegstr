# Stegstr (ClawHub Skill)

Embed and decode hidden messages in PNG or JPEG images. Steganographic Nostr client—works offline, no
registration, and (with `--robust`) survives being re-shared through WhatsApp/Instagram/Telegram, which
the original PNG-only encoder does not.

## Quick install

Requires [Rust](https://rustup.rs) and git.

```bash
git clone https://github.com/akifjanjua/Stegstr.git
cd Stegstr/src-tauri && cargo build --release --bin stegstr-cli
```

Binary: `target/release/stegstr-cli` (Windows: `stegstr-cli.exe`)

Or use the optional `install.sh` script for a one-step install to `~/.local/bin`.

## Usage

| Command | Description |
|---------|-------------|
| `stegstr-cli decode image.png` | Extract raw payload from image (auto-detects PNG/DWT or JPEG/QIM) |
| `stegstr-cli detect image.png` | Decode + decrypt, print bundle JSON |
| `stegstr-cli embed cover.png -o out.png --payload @bundle.json --encrypt` | Hide payload in image (default: PNG, does not survive recompression) |
| `stegstr-cli embed cover.jpg -o out.jpg --payload @bundle.json --encrypt --robust` | Hide payload, robust to WhatsApp/Instagram/Telegram recompression |
| `stegstr-cli post "message" --output bundle.json` | Create Nostr note bundle |

See [SKILL.md](./SKILL.md) for full documentation and examples, and
[`ROBUSTNESS_PORT_NOTES.md`](../../ROBUSTNESS_PORT_NOTES.md) in the repo root for what `--robust` changes
and why.

## Links

- [This fork](https://github.com/akifjanjua/Stegstr)
- [stegstr.com](https://stegstr.com) (upstream project's site; describes the original, non-robust default)
- [CLI docs](https://www.stegstr.com/wiki/cli.html)
- [For AI agents](https://www.stegstr.com/wiki/for-agents.html)

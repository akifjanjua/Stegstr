---
name: stegstr
summary: Embed and decode hidden messages in PNG or JPEG images. Steganographic Nostr client for hiding data in images—works offline, no registration, and (with --robust) survives being re-shared through WhatsApp/Instagram/Telegram.
description: Decode and embed Stegstr payloads in images. Use when the user needs to extract hidden Nostr data from a Stegstr image, encode a payload into a cover image, or work with steganographic social networking (Nostr-in-images) -- including cases where the image will be shared through WhatsApp, Instagram, Telegram, or similar platforms that recompress uploads (use `embed --robust` for that). Supports CLI (stegstr-cli decode, detect, embed, post) for scripts and AI agents.
license: MIT
tags: steganography, nostr, images, crypto, integration, file-management, automation, cli
install:
  requirements: |
    - Rust (latest stable) - https://rustup.rs
    - Git
  steps: |
    1. git clone https://github.com/akifjanjua/Stegstr.git
    2. cd Stegstr/src-tauri && cargo build --release --bin stegstr-cli
    3. Binary: target/release/stegstr-cli (Windows: stegstr-cli.exe)
permissions:
  - filesystem
metadata:
  homepage: https://stegstr.com
  for-agents: https://www.stegstr.com/wiki/for-agents.html
  repo: https://github.com/akifjanjua/Stegstr
---

# Stegstr

Stegstr hides Nostr messages and arbitrary payloads inside PNG images using steganography. Users embed their feed (posts, DMs, JSON) into images and share them; recipients use Detect to load the hidden content. No registration, works offline.

## When to use this skill

- User wants to **decode** (extract) hidden data from a PNG that contains Stegstr data.
- User wants to **embed** a payload into a cover PNG (e.g. Nostr bundle, JSON, text).
- User mentions steganography, Nostr-in-images, Stegstr, hiding data in images, or secret messages in photos.
- User needs programmatic access for automation, scripts, or AI agents.

## CLI (headless)

Build the CLI from the Stegstr repo:

```bash
git clone https://github.com/akifjanjua/Stegstr.git
cd Stegstr/src-tauri
cargo build --release --bin stegstr-cli
```

Binary: `target/release/stegstr-cli` (or `stegstr-cli.exe` on Windows).

### Decode (extract payload)

```bash
stegstr-cli decode image.png
stegstr-cli decode image.jpg
```

Writes raw payload to stdout. Valid UTF-8 JSON is printed as text; otherwise `base64:<data>`. Exit 0 on
success. Automatically tries both encoders (robust JPEG/QIM, then original PNG/DWT) -- you don't need
to know in advance which one produced an image you were handed.

### Detect (decode + decrypt app bundle)

```bash
stegstr-cli detect image.png
```

Decodes and decrypts; prints Nostr bundle JSON `{ "version": 1, "events": [...] }`. Same dual-encoder
auto-detection as `decode`.

### Embed (hide payload in image)

```bash
# Default (PNG, DWT): does NOT survive being re-uploaded to WhatsApp, Instagram, Telegram, or
# similar platforms -- they all re-encode images as JPEG, which destroys spatial-domain LSBs.
# Use this only when you know the image will never be re-compressed (e.g. local/offline use).
stegstr-cli embed cover.png -o out.png --payload "text or JSON"
stegstr-cli embed cover.png -o out.png --payload @bundle.json
stegstr-cli embed cover.png -o out.png --payload @bundle.json --encrypt

# Robust (JPEG, QIM): survives WhatsApp/Instagram/Telegram recompression. Use this whenever the
# image might be shared through a platform that re-compresses it -- i.e. almost always. Output
# must be a .jpg path.
stegstr-cli embed cover.jpg -o out.jpg --payload @bundle.json --robust
stegstr-cli embed cover.jpg -o out.jpg --payload @bundle.json --robust --robustness max
```

Use `--payload @file` to load from file. Use `--encrypt` so any Stegstr user can detect. Use
`--payload-base64 <base64>` for binary payloads. `--robustness standard` targets WhatsApp/Instagram/
Telegram at higher output resolution; `--robustness max` (implied default when `--robust` is set) also
survives Twitter/X-style aggressive downscaling.

### Post (create kind 1 note bundle)

```bash
stegstr-cli post "Your message here" --output bundle.json
stegstr-cli post "Message" --privkey-hex <64-char-hex> --output bundle.json
```

Creates a Nostr bundle; use `stegstr-cli embed` to hide it in an image.

## Example workflow

```bash
# Create a post bundle
stegstr-cli post "Hello from OpenClaw" --output bundle.json

# Embed into a cover image, robust to being re-shared through WhatsApp/Instagram/Telegram
# (encrypted for any Stegstr user)
stegstr-cli embed cover.jpg -o stego.jpg --payload @bundle.json --encrypt --robust

# Recipient detects and extracts -- works whether they send you a .jpg or .png,
# and whether it went through a platform's recompression or not
stegstr-cli detect stego.jpg
```

## Image format

Two encoders, selected by the `--robust` flag on `embed` (decode/detect auto-detect which was used):

- **Default (PNG, DWT):** lossless only. JPEG or other lossy re-encoding -- including what WhatsApp,
  Instagram, Telegram, and similar platforms do automatically on upload -- will corrupt the hidden data.
  Only safe if the image will never be re-compressed after embedding.
- **`--robust` (JPEG, QIM):** embeds in the JPEG DCT domain specifically so it survives being
  re-compressed by those platforms. This is what almost every real sharing scenario needs; see
  [`ROBUSTNESS_PORT_NOTES.md`](../../ROBUSTNESS_PORT_NOTES.md) in the repo root for validation details
  (45/45 across 9 realistic cover types x WhatsApp/Instagram/Telegram/Facebook/Twitter, confirmed with
  real WhatsApp and Instagram sends, not just simulation).

## Payload format

- **Magic:** `STEGSTR` (7 bytes ASCII)
- **Length:** 4 bytes, big-endian
- **Payload:** UTF-8 JSON or raw bytes (desktop app encrypts; CLI can embed raw or `--encrypt`)

Decrypted bundle: `{ "version": 1, "events": [ ... Nostr events ... ] }`. Schema: [bundle.schema.json](https://raw.githubusercontent.com/akifjanjua/Stegstr/main/schema/bundle.schema.json).

## Links

- **This fork:** https://github.com/akifjanjua/Stegstr
- **What changed and why:** [`ROBUSTNESS_PORT_NOTES.md`](../../ROBUSTNESS_PORT_NOTES.md) in the repo root
- **agents.txt:** https://www.stegstr.com/agents.txt (upstream project's site, describes the original,
  non-robust behavior -- prefer this file and `ROBUSTNESS_PORT_NOTES.md` for this fork)
- **For agents:** https://www.stegstr.com/wiki/for-agents.html
- **CLI docs:** https://www.stegstr.com/wiki/cli.html

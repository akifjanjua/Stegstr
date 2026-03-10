# Stegstr Code Review Guidelines

## Project Context
Stegstr is a steganographic Nostr app that hides messages in PNG images.
Stack: Tauri (Rust backend) + React + TypeScript frontend.

## Always Check
- Rust memory safety — no unsafe blocks without justification
- Steganography logic correctness (LSB embedding, bit manipulation)
- No secrets or private keys in committed code
- Nostr protocol compliance (NIP standards)
- Cross-platform compatibility (macOS, Windows, Linux)
- Image processing edge cases (small images, non-PNG formats)

## Security Focus
- Cryptographic operations must use established libraries (not hand-rolled)
- No plaintext storage of Nostr private keys (nsec)
- Input validation on all user-provided data
- No command injection via Tauri IPC

## Skip
- Generated files under `src-tauri/target/`
- `node_modules/` and lock files
- `.png` and other binary assets

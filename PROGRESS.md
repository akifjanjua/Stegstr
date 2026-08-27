# Progress

Status snapshot for the `hardening` branch, `STEGSTR_ENTRY_V3.md` campaign.

- **Phase 1 (break it):** 8 numbered bugs found and fixed (4 Critical, 2
  High, 2 Medium), all with regression tests. Two tiers, kept distinct:
  5 confirmed pre-existing in pristine upstream (#1, #2, #4, #5, #6, each
  verified by building and running a pristine clone or diffing against
  it), 3 introduced by this fork's own new work (#3, #7, #8 -- bugs in
  code upstream doesn't have at all). One security issue in each tier
  (#6: spoofable events; #8: crash on malformed input). See `BUGS.md`.
- **Phase 2 (environments):** Windows/macOS/Ubuntu release builds green in
  real CI (manually dispatched, run `32971568821`). MSRV 1.88.0 declared and
  actually verified. 8 npm vulnerabilities fixed (1 critical). Broken
  mobile-android submodule (failing every CI checkout) removed and
  documented. Outdated GitHub Actions bumped. `.claude/` audited (see
  git history for anything contest-strategy-specific, kept out of the
  working tree). Node 18/20 matrix configured but never actually executed
  (see `ROBUSTNESS_REPORT.md`, "what we did not test").
- **Phase 3 (networking under failure):** Nostr event signature
  verification added (bug #6, confirmed pre-existing upstream gap).
  Verified not overly strict: 100/100 real relay events accepted. Partial
  publish success now reported honestly. Reconnect-with-backoff added.
  7 failure modes tested against a local mock relay. Two gaps found and
  left open, documented: no `CLOSED`/`NOTICE` handling, no offline outbox
  for non-zap events.
- **Phase 4 (steganography):** per-cover adaptive QIM delta shipped
  (flat covers: worst-case SSIM 0.662 -> 0.758, zero survival cost,
  45/45 held). Full non-monotonic tradeoff curve documented, not picked
  quietly. One residual gap: literal solid-color cover still fails the
  harshest simulated platform.
- **Live platform confirmation:** Instagram genuinely recompresses and
  survives (+9.1% size, decodes correctly). WhatsApp comes back
  byte-identical -- a control test (untouched original through the same
  path, ~76% smaller) proved this is pass-through, not recompression
  survival, and the report says so precisely rather than the easier,
  looser claim. Telegram still simulated only.
- **Phase 5 (package):** this file, `ROBUSTNESS_REPORT.md`,
  `scripts/verify.sh` / `.ps1`, `scripts/demo.sh` all done this round.
  `run_matrix_realistic.py` already finishes in ~10s (no fix needed).

**Open items, in priority order:** Telegram live send, video recording
(shot list in `STEGSTR_VIDEO_ENTRY_BRIEF.md`), entry description text.

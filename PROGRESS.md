# Progress

Status snapshot for the `hardening` branch, `STEGSTR_ENTRY_V3.md` campaign.

- **Phase 1 (break it) + post-Phase-4 regression pass:** 9 numbered bugs
  found and fixed (4 Critical, 2 High, 3 Medium), all with regression
  tests. Kept distinct, not merged into one number: 5 confirmed
  pre-existing in pristine upstream (#1, #2, #4, #5, #6, each verified by
  building and running a pristine clone or diffing against it), 3
  introduced by this fork's own new work (#3, #7, #8 -- bugs in code
  upstream doesn't have at all), and 1 (#9) that's neither -- the faulty
  code is identical to upstream's, but the bug is only reachable because
  of this fork's own dual-encoder decode path. One security issue in two
  of the tiers (#6: spoofable events, upstream; #8: crash on malformed
  input, this fork's own work). See `BUGS.md`.
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
- **Merged to `main` and released.** `hardening` fast-forwarded into
  `main`; tag `v0.1.0` cut, `release.yml` green on all 3 OSes, GitHub
  Release live with 6 assets. Windows `.exe` verified end-to-end
  (installed, launched, real window, bundled CLI round-tripped, then
  cleanly uninstalled). Linux AppImage checksum-verified but not
  launch-tested (no Linux environment available here).
- **Post-Phase-4 regression pass (time-boxed):** adversarial corpus
  re-run against the current binary (84 ops, 0 crashes -- found bug #9
  above); dual-format decoder attacked directly with 300 trials of random
  header/body noise plus a truncated-cover case (0 panics); `verifyEvent`
  attacked with 41 malformed-event shapes (all rejected cleanly, none
  threw); diff audit of `origin/main` vs. upstream for debug prints/TODOs/
  scratch files (clean). One adjacent finding, not a code bug:
  `Stegstr_Contest_Entry.pdf` is a real committed document, predates this
  campaign, and is now stale (pre-hardening numbers, missing the bug
  count, missing the WhatsApp pass-through distinction) -- not rewritten
  here, flagged for whoever finalizes the submission text.

**Open items, in priority order:** decide what to do with the stale entry
PDF, Telegram live send, video recording (shot list in
`STEGSTR_VIDEO_ENTRY_BRIEF.md`), entry description text.

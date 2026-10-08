# LOO-428 integrated context-launch repair — 2026-10-08

Failed head `92a962b1125fa727d84071ca432fe4e03bf9adac`, CI run 37801008817:
the newly integrated context-launch fixture still used removed `--tui` and its
Codex stub did not model the parse-only trust probe. Update it and the manual
context-check script to `-i`, preserving failed-Session, large-context and no-retry
assertions. TESTING.md now includes the integrated context suite for launch changes.

Task installation failed before any test: Docker did not respond to `info` within
10 seconds, and no proof container was created. No product failure was observed;
the new-head CI run must establish installation acceptance. Do not weaken its check.

Checks: original Rust failure reproduced; isolated `context_launch_tests` and
`default_conversation_tests`, sync's focused Flow-output test, fmt, Clippy, Ruff
check/format and diff check PASS. No remaining `--tui` caller outside the rejection
test. Jack Heart's landing and Task-completion authorization remains in effect;
re-arm with `lf arm -c`. Actual cmux tracking remains unverified.

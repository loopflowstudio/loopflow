# Verify LOO-992 (2026-11-22)

PR #1984 merged as b7b6c2e087. Checkout tree identical to origin/main (`git diff --stat HEAD origin/main` empty).

- merge_group CI: success. push macOS: success. push CI: success.
- `git grep` on origin/main: no 20,000-char truncators, no DiffTier/gather_diff_tiered; commit.rs:268 and pr.rs:536 read `Change::read(...).prompt_section()`.
- `cargo test -p loopflow-execution --lib prompt::change::` → 6 passed.
- Demo, main build, isolated LF_HOME, temp repo: staged 317,782-byte `aaa_schema_fixture.json` + 6-line edit to `src/zzz_export.rs`; `lf commit -a claude:haiku` wrote
  "Retry export once when send() returns an error" — body describes the export change and mentions the fixture as generated.

Outcome: Verified.

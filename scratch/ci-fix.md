# PR #1526 — CI repair

Failed head `636ff8ecd8d4e14feb8fd857b5d88c33aac6ba6a`, run `38113601416`: 10 Rust failures; other matrix jobs passed. Sync retained that head. Previous obsolete-test repair remains intact.

- Library reconciliation re-executed its integration-test binary with the CLI-only worker flag. Reproduced locally; select the existing pinned lf for library callers while an lf CLI keeps its own executable. Existing PR and GitHub-cache integration tests exercise the real worker protocol.
- Session evidence/backfill tables are cleanup-only projections, not displayed state. Classify them as never displayed in revision coverage; no UI-invalidating triggers or second migration.
- Linux fallback disable saw a busy maintenance lock. The original race did not recur in the isolated Linux replay; descriptor inheritance is a concrete retention mechanism. A scope-owned lock now explicitly unlocks on drop, with a deterministic duplicate-descriptor proof rather than a timing retry.
- Landing fixtures expected removal during a live observing command and the old cleanup-error wording. Reproduced retention of that command. Assert retained checkout/branches, successful durable completion, and intact dirty notes; do not weaken live-process protection or restore remote-branch deletion.

TESTING.md includes the library/CLI reconciliation suites, revision classification and Linux lock proof in future cleanup gates. No ignored tests, added retries or new cleanup policy.

Check: `cargo test -p loopflow --lib -- repository_tick_admission_ends_even_with_an_inherited_descriptor repository_fallback_is_throttled_and_respects_disable store_revisions_cover_every_table` (3 passed) and `cargo test -p loopflow --test pr_tests --test land_tests --test task_github_cache_tests` (59 passed) passed on network-isolated macOS and an isolated Linux source snapshot with materialized migrations; Linux also passed the runner’s two baseline schema checks. `cargo fmt --all -- --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings` and `git diff --check` passed; new-head full CI remains pending.

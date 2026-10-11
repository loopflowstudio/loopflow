# Commit failure diagnostics

Jack Heart requested this repair on 2026-10-11; delivery is publication for
review, without landing (accepted steer `60b0eaf2`). Reconciled 2026-10-11
against implementation `c28b7d0197`; no product decision remains open.

Selected-path commits must save untracked scratch in ordinary Task checkouts,
preserve unrelated staged bytes, and retain persistent workspaces' local scratch
policy. No commit failure may print an empty error. Git failures must retain
stderr or stdout and identify silent failures
by exit status. Reproduce only in disposable repositories.

The disposable installed CLI reproduced exit 1 with `Error:` alone; the initial
regression failed with `Message("")`. The branch CLI now saves both selected
scratch files and explains an unchanged selection. No LOO-462 files were touched.
The old selection excluded scratch in every checkout, then discarded Git's stdout
when the empty commit failed. Untracked directories were not the underlying limit.

Review removed duplicate Git command/error paths. Selected commits now share
ordinary commits' identity handling and diagnostics while retaining their separate
index. Persistent scratch policy and unrelated staged bytes remain covered.
Failed staged reads now propagate shared Git errors. The deleted local runners,
staged-read helpers and duplicate message test need no replacement.

Remaining: gate's broader affected-suite verification and the caller's publication
step. The shared Git formatter affects callers beyond commit, so the seven focused
tests do not establish broader Git compatibility. No landing is authorized.

Checks: `cargo test -p loopflow --test commit_tests -- --skip commit_generates_message_when_none` passed (7); `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` passed; prior disposable branch CLI passed. Agent invocation is excluded from this focused suite; broader suites belong to gate.

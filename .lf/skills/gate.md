---
extends: builtin
docs: scripts/testing.md
---
## Loopflow candidate preparation

Use the included `scripts/testing.md` to map changed paths to tests. CI skips
its suites while the branch has files in `scratch/` (`.github/CI.md`), so this
repository's preparation, at the first publish and after review fixes, is:

```bash
lf commit -m "gate: checkpoint"     # working notes are in history here
lf sync                             # last thing before checks & publish
# When Rust changed:
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings
# When scripts/test.py --list selects ios, or rust/loopflow-model/ changed:
scripts/test_desktop.sh --filter DTOFixtureTests
uv run python scripts/test_ios.py
# When swift/ changed, also:
uv run python scripts/check_swift_multiplatform_boundaries.py
lf scratch clear                    # branch-added files only; base files stay
lf pr publish                       # ready PR, no merge request
lf pr checks                        # rust-test, tests-result: not skipping
```

If a local check fails, fix and commit before `lf pr publish`. After review
fixes, commit, run `lf sync`, re-run the applicable checks above, confirm
`scratch/` still holds only `.gitkeep` (a sync can restore notes), and
`lf pr publish`.

Before clearing, put what a later reader needs into the PR body: what people
said, what a run showed, open decisions, deferred checks and the checkpoint
SHA. The plan stays in this conversation and in
`git show <checkpoint-sha>:scratch/<file>`.

After clearing, write nothing under `scratch/`: the next publish would commit
it and CI would skip again. Findings and check results go in the report and PR
body.

If gate ends with work it cannot finish, restore the notes with
`git revert --no-edit <clear-commit>` and don't publish that commit. Local HEAD
is then a checkpoint for the next run, while the PR and its checks stay on the
candidate head: report the two separately.

Name modules in a local test filter so it finishes well inside nextest's
twenty-minute run limit (`.config/nextest.toml`). A test that fails in a loaded
run and passes alone failed from load: name it in the report and leave it to
CI.

Commands belong in `scripts/testing.md`; module checks belong in the owning
module README.

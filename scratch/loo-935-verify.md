# LOO-935 verify (main 1b888be85b, PR #1958 merged 2026-11-18T04:37Z)

Post-merge CI for 1b888be85b:
- push `CI` 44238654316: success
- push `macOS on main` 44238654290: in progress when this ran, so not observed
- merge_group `CI` 44237479693: success (queue, separate)

On main, isolated LF_HOME/HOME, LF_* unset:
- `scripts/schema_page.py --check` exit 0; `check_migrations.py` exit 0
- `scripts/due_removals.py` lists LOO-730, LOO-836, LOO-928, LOO-954, each "drop after 2026-12-18"
- LOO-954 "Drop the store members LOO-935 marked leaving" exists, unstarted
- `lf task view` → unrecognized subcommand; `lf open --follow`, `lf resume --start-if-free` → unexpected argument; `lf workspace` → skill or graph not found
- `lf open --help` has --workspace/--focus/--available and no --diff
- `open.rs` `overview()` rejects --workspace on a Task and names `lf open --wave <w> --workspace`
- `git grep task_layout` finds nothing

Finding → follow-up 144a9e: `lf open --diff patch` is accepted by the root skill-context `--diff` and ends in "No agent selected … retry with lf open loopflow://loopflow" instead of an unexpected-argument error.

Not checked here: typing from two views, focus taking the size, native-client takeover. These need held terminals or Desktop; they rely on CI's suites and the QA run named in the PR.

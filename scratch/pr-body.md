Source builds and installed development builds could send commands to separate stores, making recorded work depend on which executable ran. Ordinary commands now use the installed CLI and main Home (`~/.lf`), with the same Home carried through Task workers, Flow steps, sessions and agent tools.

<!-- loopflow:task-pr-context:start -->
> [!NOTE]
> **Task:** [One main Home that every lf uses · LOO-342](https://linear.app/loopflow/issue/LOO-342/one-main-home-that-every-lf-uses)
> **PR lifecycle:** Merging PR 1 completes the Task.
<!-- loopflow:task-pr-context:end -->

## What changes

- Forward source CLI commands to the installed CLI before opening a store. Retired `LF_CONTROL_*` pins no longer select another store or executable.
- Remove automatic branch database copies, local-build promotion and development-store recovery. `scripts/install.py local` only builds into `local-bin/`.
- Keep experiments explicit through `LF_HOME`. They initialize once and require a matching schema on later opens; schema changes require a fresh Home.
- Preserve published installation, temporary-snapshot validation and main-store migration recovery. Existing development selections route ordinary CLI launches through their published fallback.

Existing side-store data is not imported. Retirement of stores still held by older processes remains deferred until after release and those processes settle.

## Checks

Added regression coverage for explicit Home continuity, schema-mismatch refusal and removed promotion commands. Default and nested routing coverage uses real source CLI processes with a simulated installed executable.

CI repair for failed head `3c18d9238cabf022ee35181c54cd7934919b184b`: session and worktree fixtures now select their own disposable Home and executable. The planning upgrade fixture uses published migration authority against a temporary shared store, preserving the rule that existing experiments never upgrade. Testing guidance now requires checking Home fixtures without an ambient `LF_HOME`.

Local verification passed:
- Materialized-source Nextest selection: 132 tests across human sessions, planning, migrations, worktrees, and explicit Home routing, with runner `LF_HOME`, `LF_CONTROL_HOME`, and `LF_BIN` unset.
- `cargo test -p loopflow --lib migration_preserves_planning_identity_and_removes_snapshot_storage` against unmaterialized drafts: 1 passed.
- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`.

Docker did not respond within 10 seconds, so the complete disposable-installation check remains with CI. Its previously failing migration proof passed locally in both materialized and draft forms. Installed release acceptance is not established.

## Try it

Suggested walkthrough, not performed for this description: after building locally, compare `local-bin/lf wave list --json` with installed `lf wave list --json`; both should show the main Home’s Waves. Then run `LF_HOME="$(mktemp -d)" local-bin/lf wave list --json` to open an empty experiment without copying main data.
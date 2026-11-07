# LOO-498 — native workspace launch

Jack Heart's November 5 correction supersedes the passive-viewer design: create a
native Rex tab in the Task worktree and run ordinary `lf resume`. The same command
construction applies to Herdr and cmux. No custom observer or input-transfer UX.
Jack requested dramatic simplification after repeated demos opened his active
Task and rendered overlapping text. Do not open, close, resume, or modify his
running conversation or native panes while implementing or testing this change.
Use isolated fixtures for validation; future hands-on demos need separate work.

## Keep

- Native host detection, fresh tabs/splits, selected worktree and Machine.
- Saved Desktop Task layouts, then config templates, then builtin layout.
- Files, Changes, Shell and Task/workflow tools. Native hosts own arrangements;
  LF persists no host workspace inventory or mutation history.
- Normal `lf resume <session>` behavior for an explicit conversation selection;
  `lf resume --task <id>` when a Task has no conversation yet.
- Started unfinished Tasks only by default. Automatic arrivals keep native focus
  and existing input ownership: show Task/workflow in the conversation slot until
  explicitly opened. This is the minimal implementation choice needed to avoid
  background `resume` moving somebody's input.
- Repository and Wave openings start no agents: only a Session with a running
  provider resumes; other Tasks show Task/workflow in the conversation slot.
- Actual working-directory reporting/inheritance for native splits and manually
  resumed conversations. No changes to normal resume's screen protocol.

## Removed — do not reintroduce

- `open/workspace/observer.rs`: launch keys, files, locks, readiness waits/retries.
- `lf session view`, its hidden observer flag, ObserverState and readiness DTOs.
- Workspace-specific passive transport, Ctrl+R transfer, transfer placement fields,
  OutcomeUnknown wrapper and the just-added read-only hint.
- Tests exclusive to that path; use native launch + normal resume tests instead.

The prior design, failed demos and superseded constraints are retained in
`21bbb00329:scratch/rex-workspace.md` and `21bbb00329:scratch/workspace-demo.md`.
No shipping or installation requested. PR #1814 remains the delivery.


Jack Heart also requested that `docs/` read as user documentation. The workspace
guide now starts with commands and describes behavior, configuration and recovery.
Design acceptance dates, testing provenance, delivery status and internal Task
ownership remain in scratch/PR evidence instead of the guide.

Jack Heart rejected the November 6 demo showing only Task & workflow. The immediate demo must be one ordinary Task conversation in a Rex tab. A status tool is not a substitute for working agent input; do not present partial layout creation as that experience. This does not remove the previously requested optional Task/workflow tool.

Jack then confirmed “okworked” after the single ordinary Claude conversation was
opened. Use that experience as the target. This feedback approves neither shipping nor
demo cleanup.

LOO-717 / PR #1811, this work's stack parent at Jack Heart's request, merged on November 7 at 04:51Z. PR #1814 now targets main and carries native opening and cwd additions only; shell lifetime and retirement arrived with #1811.

Jack Heart requested sibling host adapters for Rex, cmux, Herdr and tmux, and one
Task diff command. Changes panes invoke `lf task diff`; that command owns Git
paging and `--difftool`. Files and Shell use internal workspace command construction;
there is no `task tool` or `desktop tool` command.

Validation of the consolidation (November 6):
- `cargo test -p loopflow --test integration task_diff_tests::`: 2 passed, including real Git difftool, pipe output and terminal paging.
- `cargo test -p loopflow --lib lf::commands::open::`: 14 passed.
- `cargo test -p loopflow --test integration work_open_tests:: -- --test-threads=1`: 24 passed, 2 native-host opt-in tests ignored. The initial overlapping build/edits and Desktop launch timeout are superseded by this final-tree run.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings`: passed. Gate/CI retain broader coverage; no new live demo or startup repair is claimed.

Jack Heart identified remaining host-specific rendering in layout.rs. Move native
wire formats, shell quoting and the Rex cwd wrapper into their adapters. Reuse
the existing Native dispatch; remove the duplicate Host renderer enum. Shared
layout code selects tools and splits only. Preserve launch behavior.

Renderer extraction checks:
- `cargo test -p loopflow --lib lf::commands::open::`: 15 passed.
- `cargo test -p loopflow --test integration work_open_tests:: -- --test-threads=1`: 23 passed, 2 native-host opt-in tests ignored; cmux caller-group lookup hit the unchanged two-second host deadline before rendering.
- `cargo test -p loopflow --test integration work_open_tests::cmux_task_joins_the_group_its_caller_is_in_now -- --exact`: passed alone using the built CLI. No timeout or retry behavior changed.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings`: passed. No live demo or fresh-conversation repair is claimed by this refactor.

Jack Heart selected internal launch code: remove `TaskCommand::Tool` and the
`lf task tool` command entirely, with no `desktop tool` replacement. Native
workspace opening builds shell/editor argv directly on the selected Machine.
Preserve configured editor/browser selection, literal file paths and checkout cwd.
Jack then wrote “otherwise lg2m ship it” after this removal, before the LOO-717
merge; see [questions.md](questions.md). Do not reopen the active conversation or
manipulate its native panes; retained demo resources have no cleanup release.

Direct launch verification: opening unit tests 17 passed; opening integration tests
24 passed, 2 opt-in native-host tests ignored; formatting and all-targets Clippy
passed. The direct-shell regression executes the generated shell command and
opens back into its Rex session with the private Machine context preserved.

## Status at 4b4c78b24

Exists: fresh-conversation startup through bare `lf open --task` inside a Rex shell, on a private Machine with one pinned runtime. Real Claude and five native tests passed on 4b4c78b24. Results and limits: [workspace-demo.md](workspace-demo.md).

Remains:
- Native Split shortcut cwd and resize: Rex app, by hand, in a separate Task.
- A dev build on the real Machine hands its child to the installed runtime until `lf install`. The cause of the November 6 exit stays unknown.
- Task with no checkout: explicit `lf open --task` places it, then opens the usual layout; see [questions.md](questions.md).
- Broad `lf open` must stop starting agents: Tasks with no running provider show Task & workflow; see [questions.md](questions.md). Correct the “send a message” line in `docs/workspaces.md`, and make a Conversation pane that fails at start visible instead of vanishing behind an `opening` receipt.
- Check, November 7: `cargo test -p loopflow --test integration work_open_tests::` passed 5 of 6 runs; the other had 2 failures, names not captured, right after clippy at load ~15. Gate reruns it and keeps failure output. `--lib commands::open::`, fmt and clippy pass.
- Shipping coverage of this tree: open in [questions.md](questions.md).

Compress restored main's #1834 (82 files) that merge `94cb745eb` dropped. Before any sync, check `git diff $(git merge-base HEAD origin/main) --stat` for files the Task never touched.

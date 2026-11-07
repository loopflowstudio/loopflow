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
- Repository and Wave openings start no agents and take no input: only a
  `detached` Session resumes; other Tasks show Task/workflow in the conversation slot.
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

Jack Heart identified remaining host-specific rendering in layout.rs. Move native
wire formats, shell quoting and the Rex cwd wrapper into their adapters. Reuse
the existing Native dispatch; remove the duplicate Host renderer enum. Shared
layout code selects tools and splits only. Preserve launch behavior.

Jack Heart selected internal launch code: remove `TaskCommand::Tool` and the
`lf task tool` command entirely, with no `desktop tool` replacement. Native
workspace opening builds shell/editor argv directly on the selected Machine.
Preserve configured editor/browser selection, literal file paths and checkout cwd.
Jack then wrote “otherwise lg2m ship it” after this removal, before the LOO-717
merge; see [questions.md](questions.md). Do not reopen the active conversation or
manipulate its native panes; retained demo resources have no cleanup release.

## Status on 746bb5fca plus the realign fix below

Exists, seen in real Rex with real Claude on 8a8ae78ef ([workspace-demo.md](workspace-demo.md)): bare `lf open --task` inside a Rex shell, on a private Machine with one pinned runtime; a Task with no checkout is placed and starts its conversation, with no second Session, worktree or replay on reopening.

Exists, fixture Rex only (built in b321745bd, Wave test 554cdc476, one `route` for opening and following 702935727):
- Broad `lf open` starts no agents. Explicit Task or Session: `Resume::Any`. Repository or Wave: `Resume::Detached`, for Task panes and the Wave's own conversation. Remote Machines receive the repository or Wave expression and judge their own Sessions.
- A Task with no conversation and no resolvable harness fails its receipt and creates no tab.
- `docs/workspaces.md` says a fresh conversation takes its first turn.

Remains:
- Demo, real Rex, isolated server, pinned branch binary: rerun the three-Task broad `lf open` from the repository, the case that failed on November 7. Expected: no new providers, no transcript growth, the ended Session not relaunched, Tasks without a detached Session showing Task & workflow, a detached Session reconnected, and a Session attached in another terminal left there with its typed text intact. Then confirm explicit `lf open --task` still resumes and starts its conversation. Add an agent-tool `lf open` of its own Task.
- Native Split shortcut cwd and resize: Rex app, by hand, in a separate Task. Jack Heart's acceptance: two Tasks in one Wave session, a new native split beside Conversation, Shell, Files, Changes and Task & workflow lands in that tab's Task worktree on its Machine, checked with `pwd` in the new shell, after reopening too; a gap in the host's scoped defaults is recorded exactly. Two-Machine behavior was not rerun.
- A dev build on the real Machine hands its child to the installed runtime until `lf install`. The cause of the November 6 exit stays unknown.
- `work_open_tests::` flake, cause now seen (November 7, load ~28, 1 of 3 parallel runs): `rex_each_open_launches_a_fresh_task_tab_without_finding_earlier_ones` failed its receipt with “Rex did not answer within 2s”. The shell stand-in missed the production reply deadline. Unrelated to this diff, and no test knob exists. Real Rex under load could report the same failure after creating the tab; the receipt says to inspect the host before retrying. Gate reruns it in parallel.

Compress restored main's #1834 (82 files) that merge `94cb745eb` dropped. Before any sync, check `git diff $(git merge-base HEAD origin/main) --stat` for files the Task never touched.

## Concept review, November 7, on 210da42d7

The model holds: `lf open` selects Work, LF renders a one-way layout, the host owns what opens. Rule the code should follow: the selection decides what resumes, and a conversation stays where its caller is. Three places break that; the first two were found by reading the code and not run, so implement writes each failing test first.

Built from the November 7 concept review (`work_open_tests::a_conversation_running_lf_open_stays_where_it_is`, `a_session_without_a_task_opens_one_conversation_tab_here`, `lf_a_reaches_only_a_tasks_first_conversation`; each failed first):

1. A pane naming `agent_caller()`'s Session shows Task & workflow, and the Wave Conversation tab is skipped. A Graph step's caller resumes as before. Realign found one more route: with no primary, `lf resume --task` ranks the Task's conversations and the caller can win, so a caller among them is treated as the named Session (`a_task_conversation_stays_put_when_the_task_has_no_primary`, failed first). This holds even if the ranking would pick another conversation; `lf open --session` still reaches that one.
2. A Session with no Task, by `--session` or `--available`, opens one Conversation tab in the caller's container, with no Task tabs and no follower. Naming the caller's own Session opens nothing. A caller in a Task checkout belongs to that Task, so `--session <itself>` there opens the Task tab with Task & workflow.
3. `--agent` reaches a pane only when `task_conversations` is empty; `remote_open` passes it on.
4. `docs/workspaces.md` says Task & workflow and that a conversation running the command stays where it is.

Kept after challenge:
- `Resume` has three values because three actors exist: a background arrival, a broad opening, a selected Task or Session.
- The follower and its `--seen`, `--remote`, `--into` and `--arrive-task` flags stay. Arrivals are accepted, and holding this state in argv is why LF records no host inventory.
- Each `lf open` creates fresh tabs, as accepted on November 5, so the Task text's “reuse/focus existing panes” no longer applies.
- Scope stays on `lf open`, with no membership record: the selection is recomputed on each opening.

Compress: `Resume::admits` is the one rule for what a pane runs, including the caller and the Wave tab; `Resolved::current_conversation` is the one caller check. `--agent` trails the argv like other optional flags. `lf -a claude open` no longer prints "--agent has no effect" (exempted in `bin/lf.rs`, not `may_launch_agent`, which also feeds `reads_work`).

## Concept review, November 7, second pass, on 746bb5fca plus the realign fix

The three rules from the first pass hold on this tree. Host routing also holds: `route()` and `Rex::local` read the current viewer before startup environment, and refuse rather than trust a stale Rex placement.

One mismatch, found by reading and not run. Only a Task Session viewer keeps a checkout (`task_viewer_holds`, `transport/README.md`). The removed passive viewer counted (“passive ones too”). Its replacement, Task & workflow in the Conversation slot, is `lf task view`, which holds nothing. So a tab from a broad open, an arrival, or an agent opening its own Task can sit open on a completed Task with no hold. About 15 minutes later, a merged Task's checkout and uncommitted files go, under a shell someone may be using.

`docs/workspaces.md` now says this under “Keep a completed Task's worktree”. Timing as the code has it: the deadline is 15 minutes from the later of completion and the last viewer leaving, a stalled owner counts as gone 30 seconds after its last report, and the once-a-minute sweep deletes in 15–16 minutes.

Checked and left alone:
- A running conversation that is not the Task's primary shows Task & workflow on a broad open. `Resume::Detached` judges the Session a pane names; the first pass kept this.
- Followers from two broad opens each add an arrival tab. Each is a separate view of the same Work, as fresh tabs accepted on November 5 imply.

Realign on 8b841cfb3: `docs/workspaces.md` retention wording read against `task_retirement.rs` and `retirement.rs`, and it agrees; no test rerun, PR #1814 head is this commit's parent.

## Concept review, November 7, third pass, on 2a26d37d3

`lf open` had its own idea of a live conversation. `Resume::Running` asked only whether the provider was alive, so a broad open performed Take back on every conversation; `session_actions` says an active Session's Open is unavailable, “use Take back to transfer it”, and “unsent text in the previous terminal is not transferred”.

Built: `Resume::Detached` asks `session_state(viewed_summary(...)) == Detached`, the read `elsewhere` and Desktop use, for Task panes and the Wave's own conversation. `provider_running` is gone; `docs/workspaces.md` updated. `repository_open_reconnects_detached_conversations_and_starts_none` and `wave_open_reconnects_its_detached_conversation_and_starts_none` failed first on the old predicate. They cover detached gets `lf resume`, an attached one gets Task & workflow, a broad open twice leaves it in the first terminal, and `lf open --task` still moves it. Fixture limit: `attach` clears the transport owner rather than running a viewer; demo owns the real one.

Consequences: opening a Wave to pull everything into Rex leaves conversations attached in Desktop there; `lf open --task` moves each. An owner that never answers reads `detached` and reattaches, same as Desktop. A viewer that lost its connection reads `active` until expiry, so nothing moves on doubt.

`lf open` and `lf resume` sharing, checked November 7 for comment d7ce2ccd: every pane runs `lf resume` through `Native::conversation`, now the one builder. `lf open` names only `session.or(primary)`, else hands `--task` to resume's ranking. `first_conversation_agent` launches nothing: it calls `Config::agent()` so a Task with no harness fails its receipt.

Compress: `Tools::conversation` reads `task_conversations` once for the caller and first-conversation checks. Check: `work_open_tests::` 32 passed single-threaded, `--lib lf::commands::open::` 17 passed. Flake: gate.

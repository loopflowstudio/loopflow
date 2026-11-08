# Plan: Task conversations start at checkout (LOO-791, PR #1844)

Design: pair session `~/.local/state/pair/sessions/852ac7bb-c097-464d-88fe-2fea18f5d941`
(round 4 `changes`/`checks`, round 5 `results`). Demo record and private Machine:
`scratch/task-session-at-checkout-demo.md`. Open choices: `scratch/questions.md`.

## Outcome

Checking out a Task starts its conversation detached beside the Graph, so
`lf resume` in the worktree reattaches instead of printing "Nothing to resume
here". A Task launch starts the provider without a repository-wide Linear sync
in front of it; the Task-only sync and comment delivery run after start. Every
native launch leaves timing in `LF_HOME/logs/launch-timing.jsonl`.

## Built (on the branch, all committed)

- `ops/task.rs` `finish_task_checkout` → `human_session::primary::start_task_conversation`:
  cap `BACKGROUND_TASK_CONVERSATIONS` (16, counted by `running_task_conversations`),
  skip when the conversation already has a transport, `transport::start` of
  `lf session resume <id>` in the worktree; failure logs a WARN and records
  `background_failed`, checkout still succeeds.
- `store/sqlite/sessions.rs` `idle_retention`: a running Task Graph holds the
  conversation against idle ending only. `lifetime.rs` reads it; `owner_retention`
  is unchanged so rollover and takeover still move the conversation.
- `lf/commands/run.rs` `prepare_task_input` no longer calls `sync_repository`.
  `harness/task_updates.rs` `watch` spawns `ops::task::sync_for_conversation`
  per Task after the provider starts, and `TaskUpdates` delivers comments it has
  not seen (`New comment from <author>: <body>`), never the conversation's own.
- `agent_input_timing.rs`: always-on log under `LF_HOME/logs`, rolled at 1 MiB
  to `.jsonl.1`, `load1` on every line; `LOOPFLOW_AGENT_INPUT_TRACE` still
  redirects. Phases added: `prompt_begin/end`, `task_sync_begin/end`,
  `background_start/skip/failed`.
- `ops/human_session/moves.rs` `send_task` waits up to 20 s for a conversation
  that is still launching before stopping it; `journal::agent_evidence` treats an
  unidentified agent whose launcher ended as dead.
- Unit tests: comment delivery and self-comment suppression, idle retention
  ending with the Graph, cap counting only running owners, log location and
  rollover, dead-launcher evidence.

## Where the implementation left the design

- Design put the running-Graph check inside `owner_retention`; built as a
  separate `idle_retention` so a Task conversation can still be rolled over or
  taken while its Graph runs.
- Design listed a `harness_prepare_begin` phase; the native launch never passes
  that step, so compress removed it. `prompt_*` and `provider_exec_begin`
  bracket the same span. The PR body still lists the phase.
- Design had the observer sync before its first delivery check and record a
  `RecordingGap` on failure; built as a thread beside the loop with an INFO log,
  so delivery never waits on Linear.
- The design's "How should a Task move treat a conversation that is still
  starting?" had no answer; the recommended option (wait up to 20 s) was built.

## Remaining

Tests from the design's check table that do not exist yet. Nothing automated
exercises the headline behavior; the integration binaries only had their
fixtures adjusted (`settings_launch_tests.rs` stand-in ignores launches without
`SETTINGS_TASK`).

1. Checkout starts the conversation: with a stand-in provider, `lf task create`
   then checkout; assert one open interactive session whose owner is running,
   and that `lf resume` in the worktree does not print "Nothing to resume here".
2. A second checkout adds no second conversation.
3. A failed start (no default agent) leaves the checkout successful and names
   the Task in `logs/lf.log`.
4. The seventeenth start is skipped (`start_in_background` with sixteen running).
5. `lf -i --task` reaches `provider_exec_begin` with a Linear stand-in that
   never answers.

Demo items not yet seen by anyone: `lf resume` reattaching to a background
conversation held by a running Graph; a comment delivered into a live
conversation. The demo page `scratch/demo.html` steps 3, 4 and 6 cover them.

## Findings

- Lib tests write into the real log. `harness::agent_process::tests`
  (`a_stop_records_where_the_conversation_went…`,
  `native_cleanup_records_wait…`) drive the spawn path without setting
  `LF_HOME`, so the compress run at 01:08Z left 20 lines (sessions `cleanup`,
  `moved`, version 0.15.1) in `~/.lf/logs/launch-timing.jsonl`. Harmless to the
  product; noise for anyone reading the log. Fix candidates: those tests pin
  `LF_HOME` to a temp dir under `journal::test_env_lock`, or the log stays off
  when `LF_HOME` is unset. Undecided.
- `lf session list` labels a Task conversation "Repository" when its Task has no
  Wave: `work_path` is computed in two places (`summary_surface` and
  `session_work_path` in `ops/human_session.rs`) and both return `None` without
  a Wave. Pre-existing on main, not caused here. A fix would show the Task
  identifier and collapse the two resolvers into one; separate Task.
- The published PR body is behind the branch on three points: the two
  integration failures (fixed in compress), `harness_prepare_begin` (removed),
  and clippy "not run" (clean). Delivery rewrites the body from its template.
- `journal::tests::a_detached_body_keeps_its_trace_after_its_launcher_is_forgotten`
  fails when run alone and passes with its module, with or without this
  branch's journal edit. Pre-existing.

## Checks

| Check | Result | Runs later |
| --- | --- | --- |
| `cargo test -p loopflow --test integration` | 532 passed, 2 failed before compress; the 2 fixed, not rerun as a binary since | gate |
| `cargo test -p loopflow --lib` modules `agent_input_timing::`, `task_updates::`, `sessions::background_task_conversation_tests::`, `harness::agent_process::tests::`, `journal::tests::` | pass (compress) | gate reruns affected |
| `cargo clippy --all-targets -- -D warnings`, `cargo fmt` | clean (compress) | gate |
| Other test binaries (`machine_commands`, `settings_launch_tests`, …) | not run as binaries after the fixture fixes | gate |
| Live demo steps 2–6 | not run by a person | demo |

# Demo: Task conversations start at checkout (2026-11-07)

Branch `jack-heart/task-session-at-checkout`, worktree
`/Users/jack/src/loopflow.task-session-at-checkout`, base origin/main `9e11b2307`,
uncommitted. Design: pair session
`/Users/jack/.local/state/pair/sessions/852ac7bb-c097-464d-88fe-2fea18f5d941`
(round 4 plan, round 5 build pages).

## What is demonstrated

1. Checking out a Task starts its conversation in a detached owner; `lf resume` in
   the worktree reattaches instead of printing "Nothing to resume here".
2. A Task launch starts Claude without a Linear sync in front of it; the Task-only
   sync runs after start (`task_sync_begin`/`task_sync_end`).
3. Launch timing is always in `LF_HOME/logs/launch-timing.jsonl` with `load1`.
4. A comment made after launch is delivered into the conversation.
5. A failed background start is logged and never fails the checkout.

## Private Machine

| Root | Path |
| --- | --- |
| Durable (Machine, history, fixture repo) | `~/.local/share/loopflow/demos/task-session-at-checkout-20261107` (`home/` = `LF_HOME`, `repo/` = fixture) |
| Runtime (copied CLI, output) | `~/.local/share/loopflow/demos/task-session-at-checkout-20261107-runtime` (`bin/lf`, `bin/demo-lf`) |

`bin/demo-lf` unsets every inherited `LF_*`, sets `LF_HOME` to the durable home and
`LF_BIN` to the copied `bin/lf`, and execs it. The real `~/.lf` is never opened.
Default agent on this Machine: `claude:haiku` (set with
`demo-lf set --global default_agent claude:haiku`).

## Observed by the agent before handing over

- Task `6e7ed5` checkout with no default agent: checkout returned in 0.7 s;
  `launch-timing.jsonl` recorded `background_failed`; `lf.log` WARN
  "background Task conversation did not start … No agent selected". (Item 5.)
- Task `603c00` checkout with `claude:haiku`: checkout returned in 0.9 s. Session
  `830fce0072434a68a0f77451412e3bef` listed `detached`, then `available`.
  Timing (seconds from `launch_lock_begin`): `fresh_launch_selected` 0.07,
  `prompt_begin` 0.29, `prompt_end` 0.58, `provider_exec_begin` 0.91,
  `task_sync_begin` 1.42, `task_sync_end` 1.44. Load average 10.7.
  Owners at 16:21 PST: session serve PID 29838 (lf process
  `a3a4a8ce-f4e8-4bb2-9a44-551c8cbfdd2e`), claude PID 29983 (agent process
  `0a8fd464-56b1-4053-9c6b-264c5be0f410`), task-updates observer PID 31991.
  With no Graph running it is expected to end after the 2-minute detached limit.
- `harness_prepare_begin` did not appear for the interactive launch; that phase is
  on the non-native harness path only. Interpretation: the native path's phases
  (`prompt_*`, `provider_exec_begin`) already bracket it.
- `lf session list` shows the Task conversation under a "Repository" column label.
  Unclear whether that is the scope column or a mislabel.

## Not yet proven

- `lf resume` reattaching to a background conversation held by a running Graph.
- Comment delivery into a live conversation.
- `cargo clippy --all-targets -- -D warnings` and the non-`integration` test
  binaries (not run). `integration`: 532 passed, 2 failed
  (`machine_commands::task_move_stops_a_live_graph`,
  `settings_launch_tests::session_and_resume_choices_do_not_select_existing_or_new_task_workers`),
  both caused by the background start; decision pending on the round 5 results page.

## Human observations

(Filled in from the conversation.)

## Teardown

Only after every draft and request in the demo conversations is settled by an
answer on the demo page:

1. `demo-lf resume <session>` in its worktree, exit Claude, confirm with
   `demo-lf session list` that no session is live.
2. Signal only a verified owner PID that will not exit (check its command line
   names the runtime root first).
3. Remove the runtime root. Keep the durable root; reopen history with
   `~/.local/share/loopflow/demos/task-session-at-checkout-20261107-runtime/bin/demo-lf session list`
   (rebuild `bin/lf` from the branch if the runtime root is gone).

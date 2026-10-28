# LOO-627 demo: New Task (2026-10-28)

Experience reviewed: starting a Task in one step, `lf task create "<request>" --run`
in the terminal and the Desktop ＋ New Task row + sheet. PR #1761
(https://github.com/loopflowstudio/loopflow/pull/1761). Design:
`scratch/loo-627-new-task.md`. Prototypes Jack chose: tab A of
`scratch/new-task-prototypes/index.html`, placement 1 of
`scratch/new-task-prototypes/placements.html`.

Status: **accepted** (2026-10-28). Jack pressed New Task in a dev build
(`~/Applications/Loopflow Dev.app`, store `/tmp/lf627/home`, repo `/tmp/lf627/myapp`),
had the sheet redesigned (see the design's "Sheet layout and finish"), saw the new
Task selected with its conversation open on its own, and said: "ok works, ship it."

Found and fixed during the demo: Start left the window unchanged. A probe showed the
only selection change came from Jack's own click; `createTask` waited for every
window reading to confirm (one `lf history` read was slow) before opening the Task.
It now opens the Task from its own read first. Test:
`newTaskSelectsBeforeReadingsConfirm` (fails on the old order).

Not shown: an agent working the request (the demo store had no Claude login;
importing Jack's was refused with 401), and `auto` reaching a merged PR.

## Setup

- Branch build `target/debug/lf` (0.15.0 + this branch, which adds no migrations).
- Fresh repository `myapp` (one commit, a `calc.py` bug) and an isolated `LF_HOME`,
  both in the session scratchpad (`…/scratchpad/demo/`), through a wrapper that clears
  inherited `LF_*`. Nothing touched Jack's store or Linear.
- Your real `~/.lf` was not used. The installed app is lf 0.14.19, and this branch is
  based on main at 0.15.0, which already includes the 0.14.20 store change. Opening
  your store with it would leave the installed 0.14.19 app unable to open it.

## Observed (agent-run, terminal)

| Action | Result |
|---|---|
| `lf task create --help` | `[REQUEST]`, `--run` and `--graph` are present, with the help text describing each. |
| `lf wave graph show --no-wave --json` | `default: {name: "code", source: "default"}`. |
| `lf task create "…" --run --graph nope` | Refused before filing: "graph \"nope\" is unavailable … Available: auto, code, …". `lf task list` is empty afterwards. |
| `lf task create "calc.add subtracts instead of adding; fix it and add a test" --run --json < /dev/null` | Returned in 2.6s. The Task `a1d61e` was filed with the request as its title. Graph `code` was started from `start`. The receipt is `{task, graph: "code", session_id}`. |
| Conversation admitted by `--run` | The `agent_sessions.request` of its primary session `04df12…` holds the typed request word for word. Graph step sessions have no request. Its cwd is the Task worktree. |
| The Graph's `implement` step | **Failed:** "no eligible managed codex account". The isolated home has no accounts, and its default agent is codex. `task run` retried it 3 times and stopped. This comes from the demo environment, not from the feature. |

Not yet observed:
- the tty path (the conversation opening in the terminal with the request as its first turn)
- a provider actually working the request
- `auto` reaching a merged PR
- the Desktop sheet

## Feedback

None yet. Jack has not run the demo.

## What would finish the proof (decision for Jack)

1. **Sign in to Claude in the demo home**, then rerun with `-a claude`, so you see a
   real run without touching your store.
2. **Update Loopflow.app to v0.15.0.** The branch build is then store-compatible
   with your real `~/.lf`, so you can try `lf task create "…" --run` and the Desktop
   build in a real repository. The same update also fixes the `.lf/tmp` sync bug.
3. **Accept the mechanics proven above** and review the Desktop sheet from the
   tests plus a later verify on main.

Recommended: 2. It's the only option that shows the Desktop half, and it removes the
`.lf/tmp` workaround.

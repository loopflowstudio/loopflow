# Demo: Task Flows without a Task worker — October 4–5, 2026

LOO-353 delivery review of the local runtime cut at `e10e2add4`, before the
October 5 pass. Design:
[focus-on-your-own-work.md](focus-on-your-own-work.md); open interpretations:
[questions.md](questions.md); code walkthrough: [pr-review.html](pr-review.html). Jack Heart's feedback is quoted below; the rest is the agent's rehearsal and
interpretation.

## What was exercised

The branch-built `lf` (0.13.2+e10e2add4) in a fresh private Home, with one
fixture Task (`INF-123`, built by the `task_flow_launch_tests` support code)
in a throwaway repository, real tmux, no provider and no Linear credential.
Flows were `cmd:`-only. The live Home was not opened or migrated.

A copy of the live store could not be used: the branch refuses it
("canonical frontier changed before 0.13.1.001_release") and custom Homes are
not upgraded. The draft migration therefore has no evidence against real data
from this demo.

## Observed (agent rehearsal)

- `lf --task INF-123 flow start proof` returned at once; the Flow ran detached
  and finished. A second start made a second FlowSession; neither continued
  the other. `task status` and `flow list --sessions --for-task` list all of
  them.
- `task restart`, `flow start --retry`, `session ready`, `session complete`
  are unrecognized commands.
- A Flow whose first step failed reads, in `task status`: "process exited with
  exit status: 1. Inspect its history and effects, then launch fresh work".
  Nothing restarted it. A later `flow start` was admitted.

## Observed by Jack Heart (October 5)

Jack ran `flow start proof` and pasted its output without comment: the launch
was admitted and printed the full Task status, with 4 Flow lines followed by
38 Exec lines. `slow` read `Current` while its own driver and step Execs read
`failed` further down.

## Jack's feedback (October 5)

- On seeing the command: "what does flow start proof even mean".
- "i am for now mostly indifferent on things like task status API changes.
  probably some good work available there, but not the current focus of the
  task."
- On learning main's #1356 removed `-b`: "Why the fuck did we do this lol".
- "this task should have been DELETING the task worker APIs and routing more
  things through the basic (e.g. flow -b) apis."

The demo did not show what Jack asked for. The cut removed the worker's
internals (claims, generations, `__worker`, restart, retry) but kept its front
door, `lf --task ISSUE flow start [FLOW]`. The agent's earlier claim that `-b`
was "unbuilt" was wrong; see direction 1.

## What the review changed

Jack's decisions and the resulting plan are in
[the design](focus-on-your-own-work.md); open choices in
[questions.md](questions.md). Two agent errors were corrected during review:

- "`-b` is unbuilt" was wrong. Main's #1356 (`a55818079`, October 1, LOO-338)
  replaced `-i`, `-b`, `--tui`, `--ide` and `commit --push` with `--mode` and
  asserted them rejected in `cli_discovery`; its message gives no reason.
- "`flow start` only checks planning and re-execs in tmux" was wrong. It
  routes to `ops::task::task_run`: placement, `--stack-on`, checkout restore,
  default Flow, agent, `--reason` steer, PR adoption/reconcile and
  working-PR guarantee, then the detached launch.

Done during review (`8847ba5ab`): `-i/-b/--tui/--ide` and `commit -p`
restored, `--mode` and `LaunchMode` deleted across Rust, Swift, tests and
docs. In the private Home, `lf -b run proof` from the Task worktree printed
the op output, blocked, and bound its Flow to the Task without `--task`;
`--mode` is an unexpected argument.

## Defects and stale surface found

1. A failed Flow stays `Current` in `flow list --sessions` and `task status`,
   beside the line saying its process exited. Only completed Flows change state.
2. `task status` prints every Exec bound to the Task: its own earlier
   inspections, `--help` calls and failed argument parses. After a dozen
   commands the Flow lines are buried.
3. With no Flow, `task status` says `action: resume`.
4. Help text: `lf task status` still says "current worker evidence";
   `lf feature` says "carry a change through its authored reviews".
5. `session list --needs-me` still exists; `--waiting` does not (known,
   unbuilt).
6. `lf flow show --sessions <id>` prints raw JSON without `--json`; its graph
   steps still carry `"human": false`.
7. `lf flow end x` answers "flow not found: end. For a skill, use `lf skill
   end`", which reads as a lookup miss, not a removed command.

## Not demonstrable yet

One Task conversation in Desktop, conversational feedback choosing the next
operational Flow, Waiting, Task primary selection, workflow files,
`loop-or-next`, taskless `-b`, all-Flow Desktop views, a real provider step, a
driver killed mid-step, and the migration against a populated store.

## After the review

The pass Jack requested is built; its state and what stays unproven are in
[the design](focus-on-your-own-work.md). Of the defects above, 1 is fixed (a
Flow whose driver failed reads `stopped`), the `lf feature` half of 4 is fixed,
5 and 7 are unchanged, and the rest were not rechecked.

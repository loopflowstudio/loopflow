# Demo: Task Flows without a Task worker — October 4–5, 2026

LOO-353 delivery review of the local runtime cut at `e10e2add4`. Design:
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

## Direction for the next implementation pass

Agent's reading of Jack's feedback, not his wording:

1. Restore `-b` on the ordinary command. It was not deleted by this branch:
   main's #1356 (`a55818079`, October 1, LOO-338 CLI owner tree) replaced
   `-i`, `-b` and their uppercase aliases with `--mode interactive|batch`; its
   message gives no reason, so neither main, the installed 0.13.0 nor this branch accepts `-b`.
   The headless path itself exists as `lf --mode batch --task <id> run <flow>`,
   which is what `flow start` re-execs.
   **Jack decided (October 5): "-b should continue to print the output and
   block just like it used to."** So `-b` is the short form of `--mode batch`,
   in the foreground. It does not detach. Agent's inference, not Jack's words:
   a caller that wants background work backgrounds the command itself (shell
   `&`, the conversation's own background-command tool), so `flow start`'s
   tmux launch and ten-second wait are deleted without a replacement. Restore
   `-i` the same way; global `-w` and `session open` are outside this Task.
2. Delete `lf flow start` and its launcher (`ops/task.rs::launch_task_flow`,
   `launch_existing_task`, `ops/run.rs::exec_task_flow`), with the
   `LF_TASK_FLOW_OPTIONS` environment hop. `lf flow` keeps `list` and `show`.
   **Correction (source read, October 5):** the agent earlier told Jack that
   `flow start` only checks planning and re-execs in tmux. It does more.
   `bin/lf.rs` routes it to `ops::task::task_run`, which:
   - places an unplaced Task (creates the worktree), applies `--stack-on`,
     restores the checkout. `lf task checkout` is the same function with
     `launch: false`;
   - resolves planning, picks the Project's default Flow when none is named,
     records a `-m` agent choice on the Task, publishes `--reason` as a steer;
   - adopts/reconciles the Task PR, refuses a dirty between-PR worktree and
     ensures a working PR;
   - then launches detached and waits up to ten seconds for the Flow row.
   The ordinary launch already covers the rest: `--task X` on any command
   enters the Task worktree (`bin/lf.rs`, `prepare_work_binding` then
   `CwdGuard::enter`), and a launch from the worktree without `--task` binds
   its Flow to the Task (observed).
   **Jack decided (October 5):** "`lf task run` or whatever is allowed to do
   #1 and #3 and then go into the lf run flow." So one thin Task entry may
   place the Task (worktree, `--stack-on`) and fill defaults (the Project's
   default Flow, the `-m` agent, `--reason` as a steer), then hand off to the
   ordinary `lf run` path in that worktree: same driver, same `-b`/`-i`
   behavior, no tmux launcher, no `LF_TASK_FLOW_OPTIONS`, no ten-second wait.
   Jack did not include #2 (adopt/reconcile the Task PR, refuse a dirty
   between-PR worktree, ensure a working PR) in what the entry may do.
   Agent's reading: it leaves the launch. Where it lives instead is open.
   Naming is open ("or whatever"); `lf task run` was itself removed by #1356
   and is still asserted as rejected in `cli_discovery`.
3. Reroute every caller onto the ordinary launch, with no alias: `task create
   --run/--flow`, Desktop Start (`MacLocalWaveAgentLauncher`, `RegistryQuery`),
   builtin skills (`task_operate`, `advance`, `launch-plan`, `repo_operate`,
   `repo_session`, `wave_operate`, `wave_session`, `s1`, `init`),
   `skills/loopflow/SKILL.md`, `docs/`, README and tests. `git grep -lE "flow
   start|LF_TASK_FLOW_OPTIONS"` lists 49 files at `e10e2add4`.
4. Status presentation (items 1-3, 6 below) is out of scope for this Task by
   Jack's statement. Leave it.

5. **Jack decided (October 5):** "running somethign from a task's worktree and
   passing --task should be equivalent ... we shouldnt get different codepaths
   or validations for either." So the planning check does not move to
   `--task`; one launch path resolves the Task from the flag or the checkout
   and applies the same checks. Observed at this commit: `lf -b run proof`
   from the Task worktree, with no `--task`, already binds its FlowSession to
   the Task. Only `flow start` runs the planning check (terminal, moved,
   removed), so today the two differ in validation.
6. Jack on `lf commit`: "should always push i guess ... or else should accept
   -p if we want to allow local only commiting." `-p/--push` is restored;
   always-push is not adopted.

## Done in this review (October 5, uncommitted behavior change)

Jack: "Definitely take this back" on `--mode`. `-i/--interactive`,
`-b/--batch`, `--tui`, `--ide` are flags again and `--mode` and `LaunchMode`
are deleted, across Rust, Swift launch argv, tests and docs. `lf commit -p`
pushes; it conflicts with explicit paths because that path has no push.
Checks: `cargo build -p loopflow --bin lf`; `cargo test -p loopflow --test
cli_discovery` 18 passed; `--lib lf::commands::run` 33 passed; in the private
Home `lf -b run proof` printed the op output and blocked, `--mode` is an
unexpected argument. Clippy clean; `flow_tests` 23, `session_lifecycle_tests` 17, `task_flow_launch_tests` 1 passed; `swift build --build-tests` built. Direction 1 is done;
2, 3 and 5 remain.

Unresolved, for Jack:

- Where PR preparation (#2 above) goes once it leaves the launch: `lf task
  checkout`, the delivery commands that need a PR (`pr publish`, `land`), or
  nowhere.

- With `-b` blocking, who backgrounds? `task create --run/--flow`, Desktop
  Start (`RegistryQuery`, `MacLocalWaveAgentLauncher`) and the operator skills
  (`wave_operate`, `task_operate`, `repo_operate`, `advance`) all rely on
  `flow start` returning at once. Proposal: delete `task create --run/--flow`;
  skills tell the agent to background the command with its own tool; Desktop
  owns the child process as it owns terminals.
- Whether the planning refusal (canceled, moved, removed Task) applies to
  every launch that resolves to a Task, or is deleted. Proposal: apply once at
  Task resolution.
- Whether `task automate`/`automation`, `task interrupt` and `task wait` are
  worker APIs to delete or ordinary Task commands to keep.

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
driver killed mid-step, and the migration against a populated store. This
review itself ran on installed `lf` 0.13.0 through the Ready/Complete handshake
the branch deletes.

## Next action

Implement the direction above. Proof: `lf --task INF-123 -b proof` in a private
Home prints the Flow's output and blocks until it ends; `lf flow start` is an
unrecognized command; `git grep "flow start"` matches only released notes and
the removed-command test.

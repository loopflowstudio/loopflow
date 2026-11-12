# Laptop and remote Machines: how Tasks are shared

Trigger (2026-11-12): MTG-321 was running `implement` on the laptop under heavy
load; the ask was "move this over to my mini". It did not move.

## Snags the move hit

| Snag | Cause | This branch |
|---|---|---|
| `Task … is unknown on this Machine; sync its planning here, then move it again` | `session receive --check` and `take_in_task` required the target's store to already hold the Task, though the TaskId is the same everywhere and planning can supply it | Fixed: the target takes the Task in from planning (`ops::task::acquire_from_planning`, shared with `prepare_task`) during the check, before anything stops |
| `lf versions differ: remote 0.15.3+3f3153d95 (build …), local 0.15.3+3f3153d95` | probe compared the full `lf --version` line with `BUILD_VERSION` | Fixed: compare the version token only |
| `ssh mini` doesn't resolve | `mini` is Loopflow's label; OpenSSH knows `mini-heart` | Documented; no change |
| plain `ssh … lf` → `lf not found` | non-login zsh lacks `/opt/homebrew/bin`; lf's own requests prepend `REMOTE_PATH` | No change; agents should use `lf --machine mini …` |
| `Remote execution uses <home> preferences; … keeps its independent edits` on every request | fires when the incoming profile conflicts and the record changed; it printed twice in a row, so "changed" may be unstable | Not investigated |

Version skew: the check now sends `--issue` and `--repository`. A mini on the
released lf rejects them, so both sides need this build before a move.

## The model as the code has it

- **Each Machine has its own store.** Nothing is replicated. Planning (Linear,
  or the Git planning journal) is the only state shared between Machines, and
  the TaskId derives from the Linear UUID, so it matches everywhere.
- **Ownership is a log.** Claim, handoff and unclaim entries are written locally
  and carried as marked Task comments. `task_execution_route` picks the writer:
  the owner's Machine, else the Wave or repository `machine.tasks`.
- **Transport is one-shot SSH.** `lf --machine` (`ssh::run_with_io`) carries the
  origin's preferences. `ssh::request` (used by move, receive and the check)
  runs with the remote's own. No server talks to another server.
- **Servers and holders are per Machine.** A Task or Wave launch is held by that
  Machine's server and holder, and fails rather than falling back to local. A
  phone attaches to one host's `lf server mobile`. Seeing a Session on mini from
  the laptop means an SSH relay to mini's viewer, which needs mini's server
  running.
- **Two transfer paths.**
  - `lf task move` / `resume --task --machine` do the full move: stop the Graph,
    checkpoint, push, carry the transcript and Graph position.
  - Pool and rental placement write only a claim and a handoff, then forward
    `lf task run`. `lf machine end` copies records home read-only.

## Expectations, as sketches

1. **Any of your Machines can take any of your Tasks without preparation.**
   Planning and the repository origin are enough; the receiver acquires and
   clones on demand. (This branch does the acquire half; clone-on-receive
   already existed.)
2. **Execution facts stay where they ran.** The laptop learns about a moved
   Task by asking its owner, not from copies. Today `lf task status` reads only
   local records, so after a move the laptop shows its own stopped history.
3. **Viewing is local-first.** Desktop and the phone talk to their own host's
   server. A remote Session is reached by relaying to that Machine's server.

## Focus

Jack, 2026-11-12: "focus on getting my personal workcase, and flexibitly to
move work between machines, at a delightful ux". Multi-person: "design for it
lightly … just not architecturally ruled out"
(`scratch/multi-user-scaling.md`). Hub and desk:
`scratch/simplification-opportunities.md`.

The case: laptop is the desk, mini is always on. Work starts on the laptop and
goes to the mini when the laptop is loaded or about to close; it comes back
when hands-on work resumes.

## What a move should feel like

1. **One act, from wherever you are.** `lf task move MTG-321 --machine mini` in
   a terminal; "move this to my mini" in any conversation; a Move to… item on
   the Task in Desktop. All three are the same operation.
2. **It lands running.** The Graph continues on the mini from the step it was
   on. Today the move stops the Graph and prints "`lf task run` there runs its
   route again".
3. **Your view follows.** The terminal or pane that showed the Session shows it
   again, now from the mini. Today the Session ends here and you run
   `lf resume --task` yourself.
4. **A running step keeps its thinking.** Today only the Task's primary
   conversation travels (`send_task`). A headless step that was mid-run, such as
   `implement`, is killed, and the step starts over on arrival from the
   checkpoint commit. Programs the agent had running (a training job) end with
   it either way.
5. **Nothing to prepare on the target.** It acquires the Task (this branch),
   clones the repository (exists), and asks for a provider login in the
   foreground if it has none, as account-selected launches already do.
6. **Progress you can read, and no half-moves.** One line per stage: checked
   mini, stopped at `implement`, pushed `<branch>`, arrived, running. A failure
   before the handoff changes nothing; after it, one retry command (exists).
7. **The laptop afterwards is clean.** `lf task status` and `lf task list` say
   "on mini since 10:42". The worktree here is removed once the push is
   confirmed. Today it stays, with a stopped Graph that reads as local history.
8. **Back is the same act.** `lf task move MTG-321 --machine laptop`, run from
   either Machine.
9. **You can see where things are.** `lf machine list` shows each Machine with
   the Tasks running on it.

## Build order

Each step is something to run and see.

| Step | You run | You see | Needs |
|---|---|---|---|
| 0 | `lf task move X --machine mini` on a Task mini has never seen | It moves | This branch, installed on laptop and mini. Not yet run for real |
| 1 | The same | The Graph is running on mini when the command returns, and this terminal shows its Session | Receive ends in `lf task run` there; the mover opens `lf resume --task`; stage lines |
| 2 | Move mid-`implement` | The step's agent continues with what it knew | Carry the step Session's transcript like the primary's, and resume it |
| 3 | `lf task status X`, `lf task list`, `lf machine list` on the laptop | Where X runs and since when; no leftover worktree | Sending retires local live rows |
| 4 | Close the lid; open the phone | The mini's Session, live | Mini as hub: Machines dial it, Desktop shows mini Tasks as ordinary rows |
| 5 | `lf task run X --rent …`, then `lf machine end` | The conversation comes home with the Task | Rental placement and give-back become moves |

Steps 0 to 3 work over today's SSH and don't depend on the hub. Step 4 is where
the transport changes.

Kept open for more than one person, at no cost now:
- Ownership entries will name the planning person; don't add new uses of the
  Git email as a key.
- The hub part reads no Work of the Machine it runs on.
- A hub connection is (person, Machine), with one person today.

## Open questions

- Should `lf machine add` / `status` check that the remote's server is
  installed? A Task launch held on mini fails without it.
- The "independent edits" preferences warning (Snags table) is still
  uninvestigated.

## Scope of this branch

Steps 0 to 3 of the build order. Step 4 is LOO-861 (hub) and step 5 is LOO-862
(rentals and pools move). Jack, 2026-11-12: "i think 0-3 could be this task,
and 4 and 5 could be follow up tasks?"

Built (uncommitted until the PR):
- Step 0: `arriving_task` / `acquire_from_planning`; `session receive --check
  --task --issue --repository`; version token comparison.
- Step 1: the move continues a Graph it stopped (`StoppedRun::resume` →
  `engine::task::run` routed to the new owner) and prints one line per stage:
  `Checked mini: it can take X`, `Stopped X at <step>`, `Committed and pushed X
  on <branch>`, `Arrived on mini (Graph at <step>)…`, `Running on mini as Graph
  process <id>`. Test: `task_move_leaves_a_live_graph_running_on_mini`.
- Step 3: `SqliteStore::tasks_owned_by`, `ops::task_ownership::runs_elsewhere`.
  `lf task status` prints `X  Runs on mini since <time>` in place of the local
  execution snapshot; `lf task list` adds it to the row; `lf machine list`
  lists each Machine's Tasks (`tasks` in `--json`, schema fixture regenerated);
  `lf wt prune` removes a checkout whose Task another Machine owns
  (`WorktreePruneReason::Moved`, "Task X runs on mini") unless it has source
  changes or a process holds it. Test:
  `the_laptop_says_where_a_moved_task_runs_and_prunes_its_checkout`.

Decided against, with the reason:
- **View follow (step 1) stays a printed command.** The departure line reaches
  the terminal as bytes the Session's holder writes (`hosted.rs` delivers
  `Settled.lines` into the terminal stream; `native_exit` does the same for a
  native launch), not as an event the viewer process can act on. Opening the
  Session from mini automatically needs a new holder→viewer message in the
  terminal protocol that Desktop panes and the phone also read, and a viewer
  that may not be a shell (a Desktop pane) would have to start an SSH relay
  itself. A laptop terminal nobody is watching would also attach to mini's
  Session, where step 4 (LOO-861) expects the phone to be the viewer. So the
  terminal keeps printing `This conversation moved to mini: lf resume --task X`.
- **A mid-run headless step restarts from the checkpoint (step 2).** No Graph
  re-run continues an interrupted step's conversation, even on one Machine:
  `RunningGraph::run_skill` (`lf/commands/graph.rs`) spawns a fresh
  `lf skill` every time, and capture creates a new Session row for it.
  Carrying the step's Session would give mini nothing to resume it with.
  Doing it takes (a) a step continuation in `run_skill`: find the prior step
  Session for the same `(node, iterations)` (the `process_input` lookup
  `limit_recovery::waiting_on_limit` uses) and, if its turn never completed,
  run `session resume <id>` in place of `skill <name>`; and (b) a second Session in
  `Move` admitted as headless (`admit` in `moves.rs` always makes the
  Task's primary and refuses a second). (a) also fixes the same-Machine case and
  is its own Task.
- Step 3 keeps the laptop's worktree until `lf wt prune`, in place of deleting
  on move.
- A repeated move after a failed receive does not continue the Graph: the
  second send finds nothing running.
- Auto-Ship approval does not travel.

Test hazard, fixed: tests that continue a run on the fixture's mini hold a
`loopflow_test_support::MachineServer` for each home and end the run. A `cmd`
step is an operation that an interrupt lets finish, so the test interrupts the
Task on mini, then signals mini's recorded `task wait` step by its verified
pid. After the run, no launchd job and no process from this checkout remained;
the fixture's two `lf holder run` processes exit after their 60 s idle.

Not approved: `lf install` on the laptop or the mini, or any real move of a
live Task. The reply to "Should I install it on both and run a move with a
throwaway Task?" was "ok, sounds good. i think 0-3 could be this task, and 4
and 5 could be follow up tasks?", which doesn't say.

## Live check (for a person, both Machines on this build)

```sh
lf install                                   # on the laptop, from this branch
                                             # and on mini, from its checkout of it
lf machine status mini                       # same version on both
lf task create 'Throwaway move check' --json  # note its identifier, X
lf task run X                                # let a headless step start
lf task move X --machine mini                # expect the five stage lines
lf task status X                             # "X  Runs on mini since …"
lf task list && lf machine list              # X under mini
lf --machine mini task status X              # Graph running there
lf wt prune --dry-run                        # "Task X runs on mini"
lf task move X --machine laptop              # back
lf task interrupt X && lf task abandon X     # clean up
```

## Checks

- `cargo test -p loopflow --test integration machine_commands::`: 43 passed.
  `documented_commands::` 4 passed; `--lib server::schema::tests:: lf::tests::
  ops::task_ownership::` 52 passed; `git::worktrees::` 22 passed; clippy clean.
- Compress: one ownership reading (`TaskOwnership::owner_elsewhere`/`since`, `MachineRef::name`) serves status, list, machine list and prune; machine_commands:: 43 passed, worktrees/task_ownership/schema lib tests passed, clippy clean.
- Gate: `test(machine_commands::) | test(documented_commands::) | test(server::schema::)`
  plus `lf::tests::` for the CLI, and clippy.
- End-to-end laptop → mini: left to a person (Live check above).

## Docs

`docs/dev/machines.md`, "Move a Task between your Machines", matches what was
built: stage lines, the printed reopen command, restart from the checkpoint,
the laptop's status/list/machine list/prune output.

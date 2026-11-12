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
   on.
3. **Your view follows.** The terminal or pane that showed the Session shows it
   again, now from the mini.
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
   "on mini since 10:42"; `lf wt prune` removes the worktree here.
8. **Back is the same act.** `lf task move MTG-321 --machine laptop`, run from
   either Machine.
9. **You can see where things are.** `lf machine list` shows each Machine with
   the Tasks running on it.

## Build order

| Step | You run | You see | Needs |
|---|---|---|---|
| 0 | `lf task move X --machine mini` on a Task mini has never seen | It moves | This branch, installed on laptop and mini. Not yet run for real |
| 1 | The same | The Graph is running on mini when the command returns, one line per stage; this terminal prints `lf resume --task X` (see Decided against) | Built |
| 2 | Move mid-`implement` | The step restarts on mini from the checkpoint commit; earlier steps of the run don't repeat (its conversation doesn't travel, see Decided against) | Built; keeping the step's conversation needs a step continuation in `run_skill`, its own Task |
| 3 | `lf task status X`, `lf task list`, `lf machine list` on the laptop | Where X runs and since when; no leftover worktree | Built |
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
- The "independent edits" warning (Snags) is uninvestigated.

## Scope of this branch

Steps 0 to 3 of the build order. Step 4 is LOO-861 (hub) and step 5 is LOO-862
(rentals and pools move). Jack, 2026-11-12: "i think 0-3 could be this task,
and 4 and 5 could be follow up tasks?"

Built (see `git log`):
- Step 0: `arriving_task` / `acquire_from_planning`; `session receive --check
  --task --issue --repository`; version token comparison.
- Step 1: the move continues a Graph it stopped (`StoppedRun::resume` →
  `engine::task::run` routed to the new owner) and prints one line per stage:
  `Checked mini: it can take X`, `Stopped X at <step>`, `Committed and pushed X
  on <branch>`, `Arrived on mini (Graph at <step>)…`, `Running on mini as Graph
  process <id>`. Test: `task_move_leaves_a_live_graph_running_on_mini`.
- Step 3: `SqliteStore::tasks_held_by` (also rentals' Task list), `ops::task_ownership::runs_elsewhere`.
  `lf task status` prints `X  Runs on mini since <time>` in place of the local
  execution snapshot; `lf task list` adds it to the row; `lf machine list`
  lists each Machine's Tasks (`tasks` in `--json`, schema fixture regenerated);
  `lf wt prune` removes a checkout whose Task another Machine owns
  (`WorktreePruneReason::Moved`, "Task X runs on mini") unless it has source
  changes or a process holds it. Test:
  `the_laptop_says_where_a_moved_task_runs_and_prunes_its_checkout`.

Step 2: the move names the live Graph's running step (`StoppedGraph { step }`
in `move.json`) and the run restarts `--from` it; same-Machine
`lf task interrupt` + `lf task run` still restarts where the run began.

Acceptance tests in `machine_commands.rs`: items 1 and 3
`task_move_leaves_a_live_graph_running_on_mini`; item 2
`the_terminal_showing_a_moved_task_says_how_to_open_it_on_mini`; item 4
`the_laptop_says_where_a_moved_task_runs_and_prunes_its_checkout`; item 8
`a_moved_task_comes_back_with_the_same_command`.

Decided against, with the reason:
- **View follow (step 1) stays a printed command.** The departure line reaches
  the terminal as bytes the Session's holder writes (`hosted.rs` delivers
  `Settled.lines` into the terminal stream; `native_exit` does the same for a
  native launch), not as an event the viewer process can act on. Opening the
  Session from mini automatically needs a new holder→viewer message in the
  terminal protocol that Desktop panes and the phone also read, and a viewer
  that may not be a shell (a Desktop pane) would have to start an SSH relay
  itself. A laptop terminal nobody is watching would also attach to mini's
  Session, where step 4 (LOO-861) expects the phone to be the viewer.
- **A mid-run headless step restarts from the checkpoint (step 2), at that
  step, without its conversation.** No Graph re-run continues an interrupted
  step's conversation, even on one Machine:
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
- The worktree stays until `lf wt prune`.
- A repeated move after a failed receive does not continue the Graph: the
  second send finds nothing running.
- Auto-Ship approval does not travel.

Test hazard, fixed: a `MachineServer` per home and an `EndRecorded` guard.

Not approved: `lf install` on the laptop or the mini, or any real move of a
live Task. The reply to "Should I install it on both and run a move with a
throwaway Task?" was "ok, sounds good. i think 0-3 could be this task, and 4
and 5 could be follow up tasks?", which doesn't say.

## Concept review (2026-11-12)

First two passes' fixes (`tasks_held_by` `since`, `lf task move` after_help)
are built. Items 3 and 4 stay as decided against above.

Third pass (2026-11-12): item 8, "Back is the same act", had no test. Built:
`a_moved_task_comes_back_with_the_same_command` moves a live Graph to mini,
then from the laptop `lf task move X --machine <laptop host name>`. It exposed
two bugs, both fixed:
- The laptop refused to continue the Graph ("is running graph hold from
  start"): a local receive ran inside `lf task move`, so the arrived Graph's
  carrier was the still-live move. `session_move::receive` now runs
  `lf session receive` as its own Process here too, as over SSH.
- Mini kept reading itself as owner: a handoff taken in from a move had no
  journal order (`ordered_at` NULL sorts last), so mini's own later handoff
  sorted before it. `ingest_ownership_marker` now orders it like a local
  append when the journal is the provider (`journal_order_in`).
Mini names the laptop `laptop` once registered there (`lf machine add laptop`
on mini). Forwarded from the laptop (`lf --machine mini task status`), the
request reads the laptop's preferences, which never name the laptop, so it
shows the Machine id. Whether the forwarded request should label its origin
is for LOO-861, which reopens what `--machine` carries.

Fourth pass (2026-11-12): done items 1 to 4 each have a test or a recorded
reason, and `docs/dev/machines.md` matches the code. Nothing to change on this
branch. One leftover for LOO-861: a Machine has a different name depending on
where you ask. To move back from the laptop you type the laptop's host name
(`local_label`), while mini calls it `laptop`. UX item 8 is written as
`--machine laptop` from either side. Choosing between one name per Machine
that travels with ownership entries and per-viewer labels is a product choice
about what a Machine is. It belongs with the forwarded-origin label above.

## Live check (for a person, both Machines on this build)

```sh
lf install                                   # on the laptop, from this branch
                                             # and on mini, from its checkout of it
lf machine status mini                       # same version on both
lf task create 'Throwaway move check' --json  # note its identifier, X
lf task run X                                # let a headless step start
lf task move X --machine mini                # expect the five stage lines;
                                             # "Stopped X at `<step>`" names the running step
lf task status X                             # "X  Runs on mini since …"
lf task list && lf machine list              # X under mini
lf --machine mini task status X              # Graph running there
lf wt prune --dry-run                        # "Task X runs on mini"
lf task move X --machine "$(hostname -s)"    # back, from the laptop
lf --machine mini task status X              # "Runs on <laptop id> since …"
lf task interrupt X && lf task abandon X     # clean up
```

## Checks

- Earlier passes (`git log scratch/remote-machines.md`): `machine_commands::`,
  `documented_commands::`, `lf::tests::`, `server::schema::tests::`,
  `git::worktrees::` passed; clippy clean.
- 2026-11-12, move back: `machine_commands::` 45 passed;
  `two_people` 1, `planning_foreground_tests:: planning_exchange_tests::` 11,
  `--lib store::sqlite::task_ownership planning_peers task_comments
  ops::task_ownership::` 15 passed; after compress, both live-Graph move
  tests; clippy clean; no launchd job or `lf` from this checkout left.
- Gate: `test(machine_commands::) | test(documented_commands::) | test(server::schema::)
  | binary(two_people) | test(planning_foreground_tests::)` plus `lf::tests::`, and clippy.
- Laptop → mini for real: Live check above, for a person.

## Docs

`docs/dev/machines.md`, "Move a Task between your Machines", matches what was
built, including moving back.

# Open choices (LOO-932)

- "One `lf` start between Start and a typeable terminal": the plan counts the
  Machine side (the held `lf session resume`, no exec into `lf -i`). Desktop's
  pane still runs a local `lf session resume ID --task X` relay to view the held
  terminal; removing it means feeding a Ghostty surface from a `session.attach`
  bridge in-app, which this plan does not do. The report counts both.
- A new Task's base is `origin/<default>` as of the last background fetch, not a
  fetch at Start (per the brief). A repository untouched for days starts days
  behind until its first sync. Taken as the simpler option.
- The ready checkout lives at `<parent>/<repo>.ready` beside Task worktrees,
  costing one checkout of disk per repository in use.
- Docs say `lf wt prune` leaves `<repo>.ready` alone, `--all` included, and
  that deleting it by hand is safe. The simpler reading of "check cleanup leaves
  it alone"; `--all` removing it would also be defensible.
- `lf wt create ready` would plan the ready checkout's path. Not guarded;
  `WorktreeSegment` could reserve `ready` if it matters.
- The ready replacement is a background `git worktree add` from setup; if the
  Machine sleeps or it is killed mid-checkout, Git leaves a locked
  registration at the ready path and takes fall back to `worktree add` until
  someone removes it (`git worktree remove --force <repo>.ready`). No repair.

Answer on the "one `lf` start" choice (Jack Heart, 2026-11-16): "I guess this is fine, im not sure what the one lf start goal is about". Counting only the Machine side and keeping Desktop's pane relay stands.

Answer on `lf wt prune --all` (Jack Heart, 2026-11-16): "Yeah that seesm right".
Answer on `lf -a X task create --run` (Jack Heart, 2026-11-16): "ok, check it".
  Checked: the conversation ran X (its Session row's provider); the Graph did not. Fixed for the CLI. Desktop's served `task.create` carries no agent; adding one waits for a picker that sends it.
- Slice 4 removes about 0.58 s (debug, loaded Machine) of exec between setup and the `-i` launch; the prompt build after it costs 1.49 s. Order kept as planned (slice 4 next); prompt build is a candidate for its own Task.
- Phone Start records no Machine kind; every phone Machine is remote to the phone.

## Concept review answers (scratch/concept-review.html, 2026-11-17)

Jack Heart: "1a 2 -- not sure but we shoudl only ever have one graph running as THE worker grapph / 3a i guess 4 no idea 5 no idea"

To build:
- 1a. Setup is owed by the Task, not the receipt. A conversation opening on a Task with no checkout, or whose taken-up Graph has never run (no GraphProcess history), runs setup. Delete `admitted_task_creation`, `record_setup_launch` and the `started` milestone on the conversation path; the receipt only answers repeats and may expire. The held argv carries the request ID for spans. Released `filed`+run rows still answer `outcome_unknown`; `-b` keeps `run_requested`.
- 2. One worker Graph per Task is already structural: `start_task_graph_run` compares the Graph row, so a racing second launch fails "moved on graph … while this run was starting". Setup must treat that loss as the Graph already running (print `Graph <name> started`, not a failure). Add a test where `lf task run` and setup race.
- 3a. A failed setup step is recorded on the Session as an observation; the Task's position reads "Setup stopped: <step> failed" on Desktop and in `lf task status`; reopening the conversation retries.
- 4a, first: one Task shape in the answer. Jack Heart: "i care more about unifying htis Three shapes of one Task in the answer ... And having simple code than i do about perfect edge/race condition checks right now (though i do still value that some)". The answer becomes receipt + session + `row` (`RoadmapTask`); Desktop fills `taskWork` from the row (extend the row only if it truly lacks a field), `overview` goes, and `TaskCreated`/`TaskCreateAnswer` become one type built where the row is read. The receipt's stored `task` stays only if the CLI's printed answer needs it.
- Priority: 4a, then 1a, then 3a. Prefer the simpler code over extra race handling; no dedicated race test for 2 unless it falls out cheaply.
- 5: no change in this Task (revisions belong to the sibling that builds pending changes).

Choices made building them (2026-11-17):
- 4a: Desktop does not fill `taskWork` from the row. The row has no interactive steps or Graph position; adding a `TaskGraph` to every listed row costs a per-Task graph read. The position line reads the row ("Starting <graph>") until the Task reading arrives; the stage dots appear with the reading.
- 1a: setup runs only for a Task that took up its Graph. A Task with no Graph and no checkout keeps the ordinary `admit_workspace` checkout (with fetch); so does a Task whose Graph has run but whose checkout is gone.
- 3a: the stop is a resumable `Failed` Task event, not a Session observation: a conversation before its provider has no input key to record an observation against, and an unknown event kind would break released lf reading the row.
- A 128-test local run (load ≈ 50) failed `task_create_answers_and_its_conversation_checks_out_without_the_network` and `session_cli_tests::a_closed_terminal_finishes_a_bare_launchs_agent`; both pass alone and the creation suite passed together. Left to gate/CI.

- `Starting <graph>` is read from the store alone (Graph taken up, never run,
  no stopped event); it doesn't check that a setup is live. A Task whose held
  conversation died before setup wrote anything (Machine restart mid-setup)
  reads "Starting code" until someone opens its conversation. Kept: "simple
  code" over "perfect edge/race condition checks right now".

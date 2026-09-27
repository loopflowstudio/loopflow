# Concept review — one owner per product object

2026-09-27 · LOO-298 · Interactive review with Jack, at `e7f3e22fd`. Replaces the
2026-09-26 review, which predates Cuts 1–G.

## Judgment

Sessions, Runs and Task invocations are rows, and every Session kind lists,
renames, binds and completes through one owner. That part of the model is clear
and should stay. The remaining defect is one concept, not a table: **a Flow run
against a Task is not the Task's Flow**, so the code keeps two Flow drivers, two
cursor owners, two review id schemes, a relaxed validator and a bind refusal to
hold that distinction up. Deciding what a Task's relationship to its invocations
is dissolves most of the remaining second implementations at once.

Findings below separate what was observed in source from what is proposed.
Nothing here re-proves the cut reports; their "Not proven" lists carry forward.

## Usage first (proposed; not verified on this branch)

```sh
lf task run INF-123                     # the Task's Flow: its Project default
lf task run INF-123 --flow incident     # replaces the Task's current Flow
lf --task INF-123 flow review-design    # a Flow about the Task; not its Flow
lf flow review-design                   # a Flow with no Task

lf session list --json                  # every open conversation, one query
lf session open  <session-id>           # resume; a dead Run gets a successor
lf session complete <session-id>        # feedback returns to the waiting step
lf flow resume <invocation-id>          # continue after review or interruption
```

Every Flow, with or without a Task, is one invocation row that owns its graph,
cursor, current attempt and failure. `lf flow resume`, `lf flow decide` and
`lf flow route` address the invocation, never a Task. A Task points at the one
invocation that advances it; other invocations may name the Task and their Runs
list under it, but only the pointed-at one is driven by the Task worker.

Recovery: a step whose Run died is retried at the same position as a new
attempt; the earlier attempt stays in history. A review whose provider died is
reopened under the same Session with a successor Run. A Flow whose store cannot
be written refuses to start, with the store error. Nothing in `flows/<id>/`
holds state; the directory holds only the driver lock.

Docs that change under this proposal: `docs/lf.md` "Running Flows" and
"Sessions" (drop the "older boundary without a Run … prepares it" paragraph at
lines 570–576, which contradicts "do not parse the Session ID" two lines up and
describes a path Cut 3 deleted); `docs/architecture/data.md` truth map (position
files row goes); `docs/architecture-reference.md` invocation-structure validator
("one current root per Task" becomes "Task's current invocation names the Task").

## Model in one screen

| User does | Type, owner | State-changing API |
| --- | --- | --- |
| Start / resume / decide / route a Flow | Invocation: graph, cursor, version, claim, failure, `current_run_id`, nullable `task_id` — `flow_invocations` | create, claim, record verdict/route, settle, end. One driver for Task and taskless |
| Run a Task | Task → its current invocation — `tasks.current_invocation_id` (proposed) | `task run` creates an invocation and moves the pointer; restart closes the old tree |
| One launch | Run: parents, provider, outcome, position, attempt — `runs` | `insert_run_in` for every launch; `end_run`; bind fills a null Task once |
| Talk: name, reopen, ready, complete | Session: title, request, feedback, completion, `current_run_id` — `sessions` | rename, ready, complete, replace current Run under a fence |
| Choose and bind Work | Task ⇒ Project ⇒ Wave | bind = write-once null-to-Task on the Session's Runs |

Session `kind` stays (completion means three different things). Session `state`
is derived from completion, feedback, clients and provider history; no column.

## Findings from source

| # | Observed at `e7f3e22fd` | Consequence |
| --- | --- | --- |
| 1 | Two Flow drivers. `controller/task/mod.rs` drives a Task's cursor through `flow_position`/`settle_task_worker` and `cursor.finish`; `lf/commands/flow.rs::drive_saved` drives a saved Flow through `FlowEngine` and `flow_run::{begin_boundary,finish_boundary,checkpoint}` on `flows/<id>/position.json`. The only production `SkillExecutor` is the saved one | Same engine, two persistence adapters selected by Task presence. This is the rule-2 defect the design names |
| 2 | `save_flow_in` UPSERTs the taskless invocation row before each headless step and resets `pending_session_id`/`current_run_id`; `position.json` remains what `recover`, `retry`, `decide`, `route` read and write | The row is a copy with no reader that the file does not also serve |
| 3 | `flow_invocations` has `UNIQUE(task_id) WHERE state='current'`; therefore `lf --task X flow foo` creates an invocation with `task_id NULL` (`flow_session::reserve`: "its Runs carry the Work's Wave and no Task") while its step Runs name X (`declared_work`) | The draft `name_tasks_on_flow_step_runs` relaxed `Run.invocation.task == Run.task` to `IS NULL OR equal`. Review Runs and step Runs of one Flow carry different Tasks. Bind refuses every Flow review because of it |
| 4 | Two id schemes for `kind='flow_review'`: `task:<inv>:<flow>:<node>:<iter>` (`review_id`) and `flow:<inv>:<boundary>` (`flow_session::session_id`); `flow_session::token` parses the latter back into a `StepToken` | The id carries state. `docs/lf.md:572` says "do not parse the Session ID"; the code does. Desktop pane keys depend on these strings |
| 5 | `lf flow decide` and `lf flow route` branch: with `LF_FLOW_STEP` they write the file; otherwise `record_flow_verdict(task_id, …)` | The fence is keyed by Task on one path and by boundary id on the other; the invocation id would serve both |
| 6 | `bind_task_worker_run` inserts the worker's Run; `claim_task_worker` does not. `task_started` still counts `worker_generation>0` | Two definitions of Started until the claim inserts the Run or the reader stops counting claims |
| 7 | Unwritable store: interactive and headless launches proceed unrecorded (`warning: this Run is not recorded`), Asks refuse, a saved Flow at a review warns and waits; the design's "One rule" says a Flow refuses | `resolve_manifest`, the `prepared` marker and `terminal.json`-as-state all survive because an unrecorded Run must still be reachable by id |
| 8 | `bind` writes without confirmation; Jack's Linear decision (`7f5c129f`) says every surface confirms the exact target once. Bind also refuses re-binding to the same Task | A retried script fails; the app has no confirm step to attach to |
| 9 | `lf session list --task` in the demo does not exist; `lf runs --task` does | Demo script needs correcting or the flag adding |

## Proposed simplification and what it preserves

**Task points at its invocation (decided).** Add `tasks.current_invocation_id`
(nullable FK), drop the partial unique index. `flow_invocations.task_id` is set for every
Flow launched with a Task, including `lf --task X flow foo`. The Run validator
returns to strict nullable equality. `task run` / `--flow` move the pointer and
close the old tree; the controller reads the pointed-at invocation.

Preserved: one Task worker at a time (the claim already lives on the
invocation); a Task-attributed Flow cannot advance the Task (it is not pointed
at); review Runs list under the Task they were run for.

**One driver.** `CliFlowExecutor` becomes the only executor; the Task controller
supplies harness, steer and attachment around it. Cursor, active boundary,
failure and completion live on the invocation row; `cwd`, `message`, `model`
become columns (launch facts); selectors resolve at launch into `task_id`/
`wave_id`, so `as_work` strings go. The step token is
`{invocation, position_version}`. `flows/<id>/` keeps `driver.lock` only.
`flow_run::{read,write,update,checkpoint,recover,retry,begin_boundary,
finish_boundary,record_decision,record_route}` and `FlowRun` delete; the import
module takes the old file shape (~15 lines).

Preserved: source-independent resume after template removal (the graph is
captured in `invocation_json`); exact process exclusion (`driver.lock`, the
claim); a failed candidate never advances (current attempt + outcome on the row).

**Opaque Session ids.** Every Session is `session_<uuid>`. A review's invocation
is `pending_session_id` on the row; nothing parses the id. Pane keys migrate
once through the import mapping, as the design already requires.

**Started = a Run names the Task.** Insert the worker's Run in the claim
transaction (`published=0` until launch), delete the `worker_generation>0`
clause and the Started event trigger once Wave chat renders from `runs`.

**Unwritable store (decided).** Every launch refuses. `resolve_manifest`, the
`prepared` marker, `run_is_prepared`, the store-unavailable warning paths and
`terminal.json` as recovery state delete in the same pass; `lf runs <prefix>`
becomes a query.

Estimated, not measured: `flow_run.rs` −430, `flow_session.rs` −100,
`controller/task/mod.rs` −200 to −300, store +150. Cut G's figure of −330/+150
covered the file only.

## Decisions for Jack

1. **Task ↔ invocation — decided by Jack, 2026-09-27.** "You should be able to
   run multiple flows at once on a task, but only one should be THE flow
   invocation for a task." Task holds `current_invocation_id`; every Flow run
   with `--task` names the Task; only the pointed-at invocation advances it and
   carries the worker claim. `lf --task X flow foo` never moves the pointer; it
   is a Flow about X, listed under X. `lf task run X [--flow foo]` is the only
   operation that sets THE invocation. Consequence to keep visible: concurrent Flows on
   one Task share its worktree, and only the managed one is fenced by the claim.
2. **Unwritable store — decided by Jack, 2026-09-27: "refuse every launch
   for now."** A Run without a row does not exist. Every launch, Ask and Flow
   step fails with the store error before any provider starts. Unlocks deleting
   `resolve_manifest` (prefix lookup becomes a query on `runs.id`), the
   `prepared` marker and `run_is_prepared` (a reserved row with `published=0`
   plus a compare-and-set), and `terminal.json` as recovery state (the row's
   outcome). `terminal.json` stays as the settlement receipt.
3. **Bind confirmation — Jack deferred to the best UX; chosen here.** The CLI
   prints the exact target and writes: a typed issue key is already a deliberate
   act, and agents and scripts call it. The app owns a confirm step, because a
   click can land on the wrong row. Binding a Session again to the Task it
   already has is a no-op success, so a retried script does not fail.
4. **Import-only columns.** Drop `historical_session_run_id` and
   `historical_ready_summary` after the installed Home imports, or keep as
   evidence. Cut G left this for Jack.

Already recorded and not reopened: taskless Flows, Session owns Runs and a
current Run, write-once bind, set-once `started_at`, no sidecars, Flow /
Invocation naming.

## Smallest next action and proof

Decisions 1–3 are made. Next, in one implementation pass with one test first: launch
`lf --task X flow <flow-with-review>`, kill the step's provider, resume, complete
the review, and assert `flows/<id>/position.json` was never written, the review
Session lists under Task X, `lf runs --task X` shows the step attempt, its
failed attempt and the review Run, and `lf flow decide` from a taskless step
records on the invocation row. Add: with the store unwritable, `lf : "x"`
exits nonzero and starts no provider; `lf session bind <s> --task X` twice
succeeds twice. The first test fails today at its first assertion.

## Carried forward, unchanged by this review

- Cut F/G "Not proven": Task step through the real binary, Wave-runner and
  PR-landing Runs, a Run whose end write fails, `lf flow resume --task` after a
  restart, `recover` under a different Home, draft migrations not materialized
  since the attempt-history rehearsal.
- Concurrent first-Home initialization race (`no such table`), observed twice.
- `pm_read_linear_oauth_sqlite_contention…` flaky, outside this Task.
- 2026-09-27 later: Jack kept Chapters/Project Flows and every redundant pair in
  LOO-298 (Cut H6/H7) and decided to delete Wave listeners, residents and lfd
  (Cut I). H1 and H2 have landed and passed review.
- Swift and DTO shapes untouched; `SessionTitleSource::Unavailable` and
  `ActiveRun.subjects` await the DTO pass. Repository Chapters and Project-owned
  Flows are not built.
- Whole-branch production delta at `e7f3e22fd`: about +1,376 (Cut G's count).

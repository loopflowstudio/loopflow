# LOO-618: `lf resume` always gets you back

Accepted by Jack Heart, 2026-10-26 (LOO-610 review, D2). Task definition is the source of truth.

## Shape

- `lf resume [ID|TASK] [--task|--wave] [--fork] [--message] [--prepare --json] [--all]`
  (canonical `lf session resume`). One verb, no open-mode flags.
  - Live lf terminal → attach and take control (transport `replace=true`); the old
    terminal says where it went (#1746).
  - A Run's id (`-b`, Graph step) → refused; points to the Task's Session (Jack Heart,
    2026-10-29; see Takeover scope).
  - Dead / never launched → resume native history or launch the prepared input.
  - Raw provider id → `provider_conversation::admit` binds a Session, then resume.
  - Scope with no conversation → start one and say so.
  - `-a` other harness → same as `lf session new -a` (handoff from `conversation_log`).
- `--fork`: a new Session in the same Work, seeded with the source's conversation log
  (`conversation_log::continuation`); the source keeps running. Harness-agnostic.
- `lf session new [ID] [--task|--wave] [--prepare --json]`: a fresh provider thread on the
  scope's same Session, seeded with the handoff; old thread stays in history. `-a` switches
  harness. Replaces `--fresh` (which created a successor Session).
- `lf task claim|unassign` replace `task defer|resume` (and this branch's `hold|release`);
  no `deferred` state (see Task assignment).
- Desktop: every preparation is `session resume <id> --prepare --json`; selection
  decides attach/elsewhere/pending from `state`/`actions`, not from a second command.

## Delete — do not maintain

- `SessionCommand::Attach`, `prepare_attachment`, `--observe`, `--if-free`
- `SessionCommand::Connect` name, `lf connect` shortcut, `--takeover`, `--try`, `OpenMode`
- `--fresh` and the `fresh` parameter through `select_connection`/`primary::ensure*`/`explain_connect`
- Primary successor admission (`replacing` in `ensure_primary_session`) — only `--fresh` used it
- `require_session_action` refusal (`OpenMode::Refuse`); `session_actions` stays for Desktop display
- `TaskCommand::Defer`/`Resume` names, then `Hold`/`Release`; `Unclaim` → `Unassign`
- `TaskState::Deferred`, `SelectionState::Deferred`, `ShippingState::Deferred`, `set_task_deferred`,
  `task_defer`, `task edit --unassign`, `tests/fixtures/dto/task_deferred.json`, Swift `.deferred`
  and `holdTask`; `WorkValue::TaskDeferred` → `HistoricalTaskPaused` (released peers still send
  `task_paused`; nothing reads it). `tasks.paused` column is unread; dropping it is a later staged change.
- `transition_headless_to_native` and the dead-headless native resume
- Swift: `prepareSessionAttachment`, `SessionOpenMode`, `--try/--takeover/--fresh` args

## Resume explanation (concept review 2026-10-28, implemented)

`lf --explain resume` answers what resume would do, in resume terms: `SessionResumeExplanation`
(`human_session/resume_explanation.rs`), intents `start|resume|take_over|new_thread`, and the
action's `agent` (selected `-a`, else the recorded provider). Explanation and opening read the
same `agent_override()` through `human_session::handoff_agent`, so `-a codex` on a Claude
conversation reports `new_thread`. Desktop perf keys `connect_prepare_*` stay: benchmark history
reads them. Desktop's `Preparation {attach, open, moveHere}` is local pane intent over the one
`resume --prepare` shape; Move here stays explicit.

## Concept review (2026-10-28)

Settled, not reopened: Desktop's Open-vs-Move-here stays explicit while CLI resume takes over
by default; Desktop has no Fork action. For gate: check help through `cli_discovery`;
`target/debug/lf help …` printed the installed app's help even with `LF_*` unset.

## Takeover scope (Jack Heart, 2026-10-29, implemented 2026-10-29)

Jack Heart decided (concept review answers 1a, 2, 3a):

- **`lf resume` takes only Sessions** (Jack Heart, 2026-10-29). Product words: Session =
  interactive, Run = headless; both are one `LfSession` record. A Run's id, running or
  finished, is refused: no takeover, no `--fork`, no interrupt hint. The refusal names the Run
  (Task · Graph · step, running/finished) and points to `lf resume --task <task>`. Same for
  `lf -b resume <id> --message` and `--prepare`. Delete `transition_headless_to_native` and
  the dead-headless native-resume path. `lf resume --all` restores interrupted Sessions only.
  Lost on purpose: continuing a finished step's own thread; the Task Session reads its log.
- **1a:** stopping a Graph is `lf task interrupt` (records the `interrupt` event; no retry).
  Resume never stops a Graph.
- **The old terminal names where it went:** "This conversation moved to Desktop" / "to ttys004"
  instead of "another terminal on this Machine" (`lf/commands/util.rs:213`), from the new
  holder's recorded terminal (`record_session_terminal`).
- `lf --explain resume <run>` reports the same refusal.
- Interactive takeover is unchanged: same provider process, control moves, turn finishes here.

## Task assignment replaces hold (Jack Heart, 2026-10-29, implemented 2026-10-29)

Jack Heart decided (answers 4a, 5a): a Task is assigned or not; there is no deferred state.

- `lf task claim <task>`: you own it on this Machine; Linear assignee = you. Unchanged.
- `lf task unassign <task>` (renames `task unclaim`): nobody owns it; **clears the Linear
  assignee** (today unclaim leaves it); nothing moves it. History intact. On a Task nobody
  owns it writes no ownership entry and still clears the assignee (decided in the second
  headless concept review from 5a and AGENTS.md Flexibility); someone else's claim refuses
  ("claimed by X; claim it to take it over"), and handoff of an unowned Task refuses.
- `lf task run` claims an unowned Task and runs (unchanged); it no longer refuses a held Task.
- Unassigned replaces deferred everywhere it was read: ship stops, CI repair skips, Wave
  operators skip (they already leave unowned backlog alone).
- Delete: `task hold/release` (this branch), `TaskState::Deferred`, `set_task_deferred` and its
  peer `deferred/resumed` planning event, `task edit --unassign` (and `--assignee` if it only
  duplicates claim), `task_row.rs` hold hint, Desktop Hold/Release (`holdTask`) → Unassign/Claim.
- Stored deferred events: ignore on read; no schema change unless a column holds the bit
  (then one draft migration). Existing deferred Tasks read as their ownership says.
- AGENTS.md Task state list drops `deferred`; skills (task/wave operate) say unassigned.

## Demo (Jack Heart, 2026-10-29: answer 6, agent-run)

Jack Heart won't run it live; the agent runs it in a throwaway `LF_HOME` with real Claude
(branch builds forward to the installed lf on the main home). Cases: live interactive
takeover (old terminal names the new one), a Run's id refused with the Task pointer,
`--fork`, dead process resumes, raw `claude` id, `session new -a`, `task claim/unassign`.

## Concept review (headless, 2026-10-29)

Implementation matches Jack Heart's 2026-10-29 decisions; no product choice open. Kept: the
Graph correction exception in `run::resume` (a Graph continuing its own step's Run). Moving
it off `resume` needs a hidden command or in-process input capture, more machinery than the
one-line ancestry check it replaces; it is invisible to people.

Re-run after the unowned-unassign change: still no product choice open. No removed spelling
(`session attach/connect`, `--takeover/--try/--if-free/--fresh`, `task hold/release/unclaim`,
`Deferred` Task state) survives outside the parse-rejection test; the remaining "headless
Session" text is a maintainer comment (`harness/claude.rs`).

## Demo findings (agent-run, throwaway LF_HOME, real Claude/Codex, 2026-10-29)

Passed: Run id refused (resume, --fork, -b --message, --prepare, --explain); bare `lf resume`
starts the repository Session; second terminal's input reaches the same Claude process;
`--fork` makes "repo (fork)" that knows the source's replies while the source stays live;
killed process → `lf resume <id>` continues the same thread; raw `claude -p` id → binds one
Session (reused on repeat) and continues its thread; `session new` and `resume -a codex` hand
off history and the old terminal prints "This conversation moved to tmux %4: lf resume <id>";
`task claim/unassign` work, `task hold` is gone.

Fixed (implement, 2026-10-29):
1. In-terminal takeover tells the old terminal: `transport::attach` sends the new holder's
   notice (`departure_notice` from `departure_here`) in `Attach.moved`; the replaced
   controller gets `Response::Moved`, prints it and is disconnected. `attach` lost its dead
   `observe`/`replace` parameters. Covered in `transport_tests` (`zero_viewers_…`).
2. `session_actions(state, interactive)`: a Run has no actions (list, surface, explain).
3. `TaskAction::Resume` → `Run` (wire `run`; Rust, Swift, fixtures).
4. `OwnershipKind::Unclaim` → `Unassign`: stored/sent as `unclaim` (released CHECK and peer
   markers), history label `unassign`, comment "unassigned this".

Demo to re-run for case 1 (old terminal prints the notice and returns to its shell).

## Remaining

- Implementation notes (2026-10-29): "unassigned" = no owner in the ownership log
  (`task_owned`); a started unowned Task goes to Later/Backlog with next move User. `lf ship`
  claims an unowned Task on start (as `task run` does) and stops when it is unassigned
  mid-ship (`ShipStep::Unassigned`); the step already running finishes.
  Desktop reads `runtime.owner` for Claim/Unassign (`RoadmapTask.isUnassigned`).
  Mid-ship unassign and the Linear assignee clear are covered by
  `task_ship_unassigned_mid_ship_waits_and_lets_its_running_step_finish` and
  `local_planning::task_unassign_clears_the_assignee`. The released 0.14.17 migration
  comment still says "Defer holds it"; it is a shipped record, so it stays.
- Checks (2026-10-29): lib `human_session:: ops::task::ship store::sqlite::{task_work,selections,
  planning_write,task_ownership,ci_incidents} work::selection lf::tests lf::commands::waves
  agent::session:: task_graph_launch_tests::task_ship`, integration `world_recipes:: dto_fixtures::
  cli_discovery:: machine_commands::task_ownership local_planning::task_unassign_clears_the_assignee` (+ `_of_an_unowned_task`,
  `task_ownership::tests::unassigning_an_unowned_task_writes_nothing`),
  `two_people`, `transport_tests` Graph-step refusal, `session_lifecycle_tests` structured
  correction, `session_launch_contract` schema refusal, Swift `WorkModel|WaveStandup|DTOFixture|
  RegistryQuery|SessionsStore|DesktopHeadlessTests`, fmt, clippy all-targets — pass.
- Checks (demo fixes, 2026-10-29): lib `human_session:: task_actions:: task_ownership::
  lf::commands::util:: durable::`, integration `dto_fixtures:: local_planning::
  machine_commands::task_ownership`, `transport_tests zero_viewers`, Swift
  `RoadmapView|DTOFixture|RegistryQuery`, fmt, clippy all-targets — pass.
- Compress (2026-10-29): lib `ops::task_ownership::`, `transport_tests zero_viewers|automatic_attachment`, fmt, clippy all-targets — pass. Kept transport's `replace`/`Busy`: no production client sends `replace=false` now, but a busy owner from the previous build still requires the field.
- Assumptions: `scratch/questions.md`.

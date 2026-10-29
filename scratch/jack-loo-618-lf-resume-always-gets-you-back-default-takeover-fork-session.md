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
- `lf task hold|release` replace `task defer|resume`. State name `deferred` unchanged.
- Desktop: every preparation is `session resume <id> --prepare --json`; selection
  decides attach/elsewhere/pending from `state`/`actions`, not from a second command.

## Delete — do not maintain

- `SessionCommand::Attach`, `prepare_attachment`, `--observe`, `--if-free`
- `SessionCommand::Connect` name, `lf connect` shortcut, `--takeover`, `--try`, `OpenMode`
- `--fresh` and the `fresh` parameter through `select_connection`/`primary::ensure*`/`explain_connect`
- Primary successor admission (`replacing` in `ensure_primary_session`) — only `--fresh` used it
- `require_session_action` refusal (`OpenMode::Refuse`); `session_actions` stays for Desktop display
- `TaskCommand::Defer`/`Resume` names
- Swift: `prepareSessionAttachment`, `SessionOpenMode`, `--try/--takeover/--fresh` args

## Resume explanation (concept review 2026-10-28, implemented)

`lf --explain resume` answers what resume would do, in resume terms: `SessionResumeExplanation`
(`human_session/resume_explanation.rs`), intents `start|resume|take_over|new_thread`, and the
action's `agent` (selected `-a`, else the recorded provider). Explanation and opening read the
same `agent_override()` through `human_session::handoff_agent`, so `-a codex` on a Claude
conversation reports `new_thread`. Desktop perf keys `connect_prepare_*` stay: benchmark history
reads them. Desktop's `Preparation {attach, open, moveHere}` is local pane intent over the one
`resume --prepare` shape; Move here stays explicit.

## Desktop Task hold (concept review 2026-10-28, implemented in b75b9a0cf)

Desktop says Hold/Release (`holdTask(_:held:)`, ids `task-hold-release`/`task-hold`) so "Resume"
means only a conversation. The status label stays "Deferred": AGENTS.md names that Task state;
this Task renames the verbs only.

## Concept review (2026-10-28, headless, after Desktop hold)

Nothing to change. Clap surface matches the accepted shape (`Resume {id, --fork, --all}`,
`New {id}`, `task hold|release`); no live caller of a deleted verb or flag outside shipped
release notes and the parse-rejection test. Session skills say `lf session new` where they said
`--fresh`. Settled, not reopened: Desktop's Open-vs-Move-here stays explicit while CLI resume
takes over by default; the state label stays Deferred; Desktop has no Fork action (acceptance
doesn't require one). Note for gate: `target/debug/lf help …` printed the installed app's
old help even with `LF_*` unset, so check help through `cli_discovery`, not by hand.

## Takeover scope (Jack Heart, 2026-10-29, accepted; next implement)

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

## Task assignment replaces hold (Jack Heart, 2026-10-29, accepted; this PR; next implement)

Jack Heart decided (answers 4a, 5a): a Task is assigned or not; there is no deferred state.

- `lf task claim <task>`: you own it on this Machine; Linear assignee = you. Unchanged.
- `lf task unassign <task>` (renames `task unclaim`): nobody owns it; **clears the Linear
  assignee** (today unclaim leaves it); nothing moves it. History intact.
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

## Remaining

- Checks (2026-10-28): lib `human_session:: lf::`, integration `session_cli_tests:: work_open_tests::
  cli_discovery:: context_launch_tests:: machine_commands::`, `session_launch_contract`,
  `transport_tests`, `global_commands`, `golden_prompt`, fmt, clippy — pass (two load-timing
  flakes in untouched fixture code pass in isolation).
- Compress (2026-10-28): `new_thread` owns its launch; unused Swift `prepareSession(fork:)` removed.
  `human_session::` lib, `session_cli_tests::`, clippy — pass (two load flakes pass alone).
- Realign (2026-10-28, after merging main #1761 New Task): no new callers of removed
  verbs; `cargo check -p loopflow --all-targets` — pass. Swift untested against #1761.
- Implement + compress (2026-10-28, concept review): `dto_fixtures::`, `global_commands
  session_resume_explain`, lib `human_session::`, `session_cli_tests::`, fmt, clippy — pass (one load flake passes alone).
- Realign (2026-10-28, after main #1764): the concept-review items are committed in 8c45266af
  (`git grep SessionConnect|explain_connect|ConnectOrResume` → only `SessionConnectionArgs`,
  the shared `resume`/`session new` Clap args, which stay). #1764 touches CI jobs only; no rerun.
- Implement (2026-10-28, Desktop hold): `scripts/test_desktop.sh --filter 'WorkModelTests|DesktopHeadlessTests'` — pass.
- Publish (2026-10-28): `scripts/test_desktop.sh --filter 'SessionsStoreTests|WorkDestinationTests|WorkModelTests|RegistryQueryTests'` — Swift build against main + 129 tests pass. `session_lifecycle_tests` left to CI.
- Assumptions: `scratch/questions.md`.

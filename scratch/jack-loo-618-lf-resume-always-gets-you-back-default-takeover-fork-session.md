# LOO-618: `lf resume` always gets you back

Accepted by Jack Heart, 2026-10-26 (LOO-610 review, D2). Task definition is the source of truth.

## Shape

- `lf resume [ID|TASK] [--task|--wave] [--fork] [--message] [--prepare --json] [--all]`
  (canonical `lf session resume`). One verb, no open-mode flags.
  - Live lf terminal → attach and take control (transport `replace=true`); the old
    terminal says where it went (#1746).
  - Live provider client lf launched but no lf terminal → stop it, resume here, say so.
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

## Remaining

- Checks (2026-10-28): lib `human_session:: lf::`, integration `session_cli_tests:: work_open_tests::
  cli_discovery:: context_launch_tests:: machine_commands::`, `session_launch_contract`,
  `transport_tests`, `global_commands`, `golden_prompt`, fmt, clippy — pass (two load-timing
  flakes in untouched fixture code pass in isolation).
- Compress (2026-10-28): `new_thread` owns its launch; unused Swift `prepareSession(fork:)` removed.
  `human_session::` lib, `session_cli_tests::`, clippy — pass (two load flakes pass alone).
- Gate: Swift build + Sessions/WorkDestination tests; `session_lifecycle_tests` (CI-only).
- Assumptions: `scratch/questions.md`.

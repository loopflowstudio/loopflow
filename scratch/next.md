# Slice 6D1: remove false Session identity

## Implement

- Rename Task workspace DTO `session_id` fields to `task_id` in Rust, Swift,
  fixtures, and tests.
- Rename `ChildControlActivity.session_id` to `work_id` across Rust/Swift wire
  mirrors and journal fixtures.
- Replace `LaunchSurfaceRecord.sessionId` (an alias of `launch.id`) with
  `launchId` in its consumers.
- Rename production Rust Project/Task locals, parameters, errors, comments,
  prompts, and help that call stable Work a Session.
- Replace user docs that advertise Task Session, Project Session, or durable
  Session identity with Task Work, Project Work, or Launch as appropriate.
- Retain explicitly named provider session ids and real tmux, Ghostty, browser,
  and human sessions. Do not redesign server topology in this pass.

## Done when

- [x] Task workspace JSON names `task_id`; child activity JSON names `work_id`.
- [x] Launch presentation consumers name `launchId`.
- [x] production Project/Task code does not call stable Work or ids sessions.
- [x] user docs contain no Task Session, Project Session, or durable Session.
- [x] Rust/Swift DTO and focused behavior tests, fmt, and all-target clippy pass.

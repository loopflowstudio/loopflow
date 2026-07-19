# Slice 6D2: name Mac projections after what they contain

## Implement

- Rename the Active Sessions projection to a Work census: `WorkCensus`,
  `WaveActivity`, `WorkActivity`, `WorkActivityKind`, and `ActivityEvidence`.
- Rename its view and row view consistently. Replace row `actions` with the
  existing optional `launchId`; delete the one-value `SessionAction` enum.
- Rename Context Lab's AgentLaunch-based contract from Session to Launch:
  `LaunchSetQuery`, `LaunchSetTotals`, `LaunchLane`, `launches`, and
  `ContextFlameLevel::LaunchSet` in Rust, JSON, Swift, fixtures, and UI copy.
- Rename agent-session count fields in that contract to launch counts. Add no
  compatibility keys or aliases.
- Preserve real provider, tmux, Ghostty, browser, and human Session vocabulary.

## Done when

- [x] no ActiveSessions/ActiveSession/SessionAction projection types remain.
- [x] openability derives from `launchId`; the action enum/array are deleted.
- [x] Context Lab wire and UI use LaunchSet/LaunchLane/launches and launch counts.
- [x] remaining current Session symbols refer to real provider or surface
      sessions, not Work, Run, Launch, or an AgentLaunch projection.
- [x] Context/DTO/UI Rust and Swift tests, fmt, and all-target clippy pass.

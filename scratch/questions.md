# LOO-507 implementation assumptions — 2026-10-17

- No prior design exists in scratch. Jack Heart's Task directive defines the
  outcome; the local plan records draft implementation choices.
- Pi is a harness, not an inference provider account. Its own login and model
  settings remain authoritative; managed Claude/Codex accounts are not reused.
- The managed execution descriptor records Git/control roots, not an OS
  sandbox. Existing Task launches bypass vendor sandboxing. Pi follows that
  same caller-owned OS boundary; adding a new sandbox service is outside scope.
- Use current Pi 1.3.0 protocol (`agent_settled`), with explicit errors for
  unsupported protocol behavior rather than old-version compatibility paths.

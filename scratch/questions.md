# LOO-507 implementation assumptions — 2026-10-17

- Jack Heart's Task directive defines the outcome; the branch design records
  draft implementation choices pending demo review.
- Pi is a harness, not an inference provider account. Its own login and model
  settings remain authoritative; managed Claude/Codex accounts are not reused.
- The managed execution descriptor records Git/control roots, not an OS
  sandbox. Existing Task launches bypass vendor sandboxing. Pi follows that
  same caller-owned OS boundary; adding a new sandbox service is outside scope.
- Use current Pi 1.3.0 protocol (`agent_settled`), with explicit errors for
  unsupported protocol behavior rather than old-version compatibility paths.

- The repair pass treats a successful final result as nonblank text with Pi's
  `stop` reason. Tool-only or thinking-only endings cannot supply a Flow answer.
- Public steering uses the launch extension and existing callback receipts because
  the current lifecycle has no production `Harness::send_current` consumer. Pi
  queues custom steering while active; no new dispatcher or store is introduced.

- October 18 gate: the changed-aware runner stopped before product checks because
  shared uv-cache inventory exceeded 15 seconds, despite 60.3 GiB free against a
  32 GiB emergency floor. Run bounded affected commands against existing artifacts;
  do not delete unrelated caches or claim the aggregate runner passed.
- Real Pi's detached tool children survive abrupt provider death. Keep the failing
  acceptance probe and block landing; ownership must be captured before reparenting.
  A new Pi process registry or postmortem PID inference would exceed gate's polish
  boundary and contradict the accepted single process owner.

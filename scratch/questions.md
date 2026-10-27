# LOO-638 assumptions (kickoff 2026-10-27)

- Any Claude/Codex `SessionStart` (startup, resume, clear, compact) in the owned
  process may reselect the native conversation; only the capability and the
  live-AgentProcess join fence the process. Simpler than per-source rules;
  revisit if a provider is found running lf hooks in a foreign process.
- `/clear` gets the full `start` context block (fresh conversation needs the
  Task brief), not a reduced one.
- Resume after `/clear` is now in scope (Jack's steer). A `Select` publishes a
  new resume reference only when the Session already has one; it carries that
  reference's account. No reference yet → the launch transport publishes.
- Observed, not fixed: `lf session connect` within ~2s of a headless run's exit
  fails with "AgentProcess group … survives its recorded leader" while the
  lifeline watchdog (TERM, `sleep 2`, KILL) is still in the group. The
  integration test waits for the group to end. Candidate separate Task.

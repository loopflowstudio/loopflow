# Open questions and assumptions · LOO-711

- **Current delivery boundary unresolved** (implementation, 2026-11-02).
  Jack Heart selected worker posts into the Session plus the run page. The
  kickoff's wait-only mechanism is superseded. Codex has an existing native
  protocol relay, but no worker-post operation; Claude's session-bus probe has
  no production transport here. Provider scope and native delivery receipts
  remain undefined. Implementation stopped before substituting a Desktop card,
  next-prompt injection or Claude-only wait for that outcome. Details and the
  current deletion list are in `loo-711-task-sessions-tell.md`.

The following kickoff assumptions and probes are historical evidence, not an
accepted delivery contract for the combined design.

- **Every move starts a turn** (kickoff, 2026-11-02). The brief left open
  whether an update should start a turn in an idle Session or wait for Jack's
  next prompt, and whether every step or only finished/failed/waiting events
  are worth one. Assumed: every change of the Task's work position starts a
  short reporting turn; Jack's complaint was silence, and the knob to quiet it
  is skill text. Reversible without code changes.
- **Interactive Claude starts a turn on a background notification while idle.**
  Resolved 2026-11-02 in the LOO-711 Task Session itself (Claude Code TUI in
  Desktop): a background `lf task wait LOO-711 --until terminal --timeout 45s`
  armed before the turn ended produced a new turn on exit, with nothing typed.
  Worlds 1 and 2 on `scratch/explore.html` rest on a proven mechanism.
- **Claude Code's same-machine session bus wakes an idle Session.** Resolved
  2026-11-02: a detached headless `claude -p` used SendMessage to this Session
  by name; the message arrived mid-turn (13:40) and, on a second probe, started
  a turn in the idle Session (13:42) with nothing typed. Rendered in the TUI as
  "Message from @probe-…". This is a push primitive for the Claude adapter with
  no re-arm and no idle lf process. It is Claude Code only (Unix socket per
  session under /tmp/cc-socks); cross-harness delivery still needs lf to own
  the event and post it where each harness can hear it.

# Session wake evidence · LOO-711

Research from 2026-11-02, gathered at Jack Heart's request during explore.
These are recorded observations, not newly verified provider guarantees.
Original research and comparisons: `5f658180d9:scratch/research-harnesses.md`;
probe details: `5f658180d9:scratch/questions.md`.

## Claude probes

- **Background completion woke an idle interactive Session** at 13:33.
  In the Claude Code TUI in Desktop, a background
  `lf task wait LOO-711 --until terminal --timeout 45s` exited and started a
  new turn with nothing typed. This proves the provider wake primitive;
  it does not establish a Task-movement subscription.
- **Same-machine SendMessage delivered mid-turn and woke an idle Session.**
  A detached headless `claude -p` addressed the Session by name: delivery at
  13:40 and an idle wake at 13:42, rendered as “Message from @probe-…”.
  The probe used Claude's session bus (`ListAgents`/`SendMessage`, sockets
  under `/tmp/cc-socks`). There is no production lf transport or stable address
  mapping here. A new model invocation per update was a probe, not an accepted
  delivery mechanism.
- **Contrary evidence:** the original research cites
  `anthropics/claude-code#86365` for background notifications dropped mid-turn
  (closed “not planned”, October 16), and `#87689` for top-level idle wakes
  versus queued subagent notifications. Re-reading state and re-arming only
  mitigates loss; it does not prove reliable delivery.
- Claude Channels were reported as an interactive research preview requiring
  an allowlisted plugin or development dialog. They were not launchable by lf
  in this investigation and were not selected.

## Lessons retained from other harnesses

The original comparison examined OpenClaw announcements, Amp thread events,
HumanLayer control flow, and Slack-native agents. Useful design lessons:
status comes from process exits; settle related events before reporting;
keep worker prose distinct from mechanical results; deliver into the existing
conversation; allow silence when nothing material changed. Those comparisons
are design references, not evidence that lf already has a native delivery path.

The current plan supersedes the research's wait-only and Desktop-badge scope.
Codex's existing native relay must be investigated on its own terms; absence of
Claude's background-command wake does not establish that Codex cannot receive
worker posts. Temporal was a presentation reference, not a replay requirement.

# How other harnesses let a session hear its background work (research, 2026-11-02)

Read for LOO-711 at Jack's request during explore. Primary sources where reachable.

## Claude Code (our substrate)
- The top-level interactive session is woken from idle by a background command
  exiting. Confirmed in this Session (probe, 13:33) and by the reporter of
  anthropics/claude-code#87689 (7 of 7 coordinator launches woke it).
  Subagents are not woken; their notifications queue until messaged.
- Same-machine session bus: ListAgents lists every Claude session on the
  Machine by name; SendMessage delivers to one. Probed 2026-11-02 from a
  detached `claude -p`: delivered mid-turn, and woke the idle Session. A push
  primitive for the Claude adapter. Unix socket per session (/tmp/cc-socks).
- Channels (v2.1.80+, research preview): an MCP server declaring
  `claude/channel` pushes `notifications/claude/channel`; events queue and are
  delivered together on the next turn. Interactive only, allowlisted plugins or
  a development flag with a dialog. Not launchable by lf today.
- anthropics/claude-code#86365: a notification that arrives while the parent is
  mid-turn can be dropped, not queued. Closed "not planned" 2026-10-16.
  Implication for world 1: a wake can be lost when the Task moves while Jack is
  talking to the Session. Safe only because every turn rereads status and
  re-arms; a lost wake costs one report, delivered on the next turn.

## OpenClaw subagents (docs.openclaw.ai/tools/subagents/announce)
The most worked-out version of what we are building. A finished child is
announced into the requester as a stable event block: source (subagent|cron),
status (ok|error|timeout|unknown, from the runtime outcome, not the child's
prose), the child's last visible text, a stats line (runtime, tokens, cost,
transcript path) and a follow-up instruction: "review the result, continue
unfinished work, and report the outcome." Top-level requesters receive a
follow-up `agent` call (a turn). Completions aggregate per requester run and a
frozen batch waits for descendants to settle before delivery. A completion turn
may end with `NO_REPLY`, which settles it without a visible reply.
Lessons: status derived from exits, not narration; settle before delivery; the
event carries what to do next; silence is a legal reply.

## Amp (ampcode.com manual and July 2026 releases)
- The person is notified by the app ("The apps notify you when a thread is
  ready for your next message"), not by an agent's prose.
- Event-driven orbs (2026-07-23): a webhook handler "is a router, not a worker";
  it appends the event to an existing thread or spawns a new one. Appending an
  event is how a thread hears about the world.
- Self-scheduling (2026-07-21): "Agents set their own schedules, wake themselves
  up, and resume with their saved prompt and full prior context."
- Agent-to-agent messaging across threads (2026-07-17); Puck (2026-07-20) is a
  meta-agent that launches and coordinates agents. How spawned threads report
  back to Puck is undocumented.

## HumanLayer / 12-factor agents
- Factor 8, own your control flow: deterministic code owns the loop; the LLM
  returns only the next step as an intent. Sync steps `continue`; async or
  human steps log, save the thread and `break`, resuming on a webhook.
- Factor 6, launch/pause/resume: "External triggers like webhooks should enable
  agents to resume from where they left off."
- Factor 7, contact humans with tool calls: the agent reaches the person in
  Slack or email; the person never polls.
- Loopflow's Graph already is this shape: `interactive:` steps are the break
  points, `lf task run` the resume, GraphProcess the saved thread.

## Slack-native agents
- Factory Droid posts progress and results in the conversation that asked, opens
  a per-task channel showing repo, branch and diff, and posts a summary with PR
  links on finish. Warp Oz and xpander's Software Factory do the same.
- Cursor cloud agents DM on completion; a forum report says long-running agents
  do not. Codex cloud has no completion notification; `codex cloud wait` is an
  open request. cmux signals attention with terminal escapes (OSC 9/777) that
  ring the pane and badge the tab.

## Pattern
Two channels everywhere, never mixed:
1. Agent to person: a push into the place the person already is (app
   notification, Slack thread, pane ring). Never prose the person must request.
2. Agent to agent: an event appended into the session that starts a turn, with
   a status derived from exits and an instruction for what to do with it.
Nobody holds a turn open to watch (world 3). Nobody makes "next prompt knows"
the primary path (world 4). World 1 is the agent-to-agent channel; the
agent-to-person channel is the Desktop badge the design excluded.

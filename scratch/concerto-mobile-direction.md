# Concerto Mobile Direction

Mobile access to loopflow agents without terminal streaming.

## Problem

Users want to manage waves from their phone. The original plan was terminal streaming—bidirectional PTY I/O over gRPC. But:

1. Terminal UX on phones is bad
2. We'd need an iOS terminal renderer (Ghostty is macOS-only)
3. We support multiple agents (Claude Code, Codex, Gemini CLI)—each has different terminal UI
4. Raw terminal access is more power than mobile users need

## Approach

Three phases, each self-contained:

| Phase | What | Mobile can do |
|-------|------|---------------|
| A | Non-interactive | Trigger steps, see status, land PRs |
| B | Chat | Discuss code, get suggestions, plan work |
| C | Agent harness | Chat + tools = full agent on phone |

### Phase A: Non-interactive mobile

Mobile is a remote control. No conversation, no back-and-forth.

**Actions:**
- See wave status (running, waiting, idle)
- Trigger non-interactive steps/flows
- Land PRs
- Create waves
- See results (what changed, PR link)

**What exists:**
- HTTP API for wave management
- Events for status updates
- Push notifications (needs implementation)

**What's needed:**
- iOS app with wave list, status, action buttons
- Push notification infrastructure
- Auth (JWT via loopflow.studio)

No terminal streaming. No chat. Just status and triggers.

### Phase B: Chat experience

Add conversation without tools. Use LLM API directly (Claude, OpenAI, Gemini).

**Actions:**
- "What's wrong with this PR?"
- "How should I approach this bug?"
- "Review this diff"
- "What does this function do?"

**Mobile sees:**
- Chat interface
- Codebase context assembled automatically
- Suggestions for steps to run

**Mobile cannot:**
- Edit files directly
- Run commands
- Make commits

Chat is for thinking and planning. Execution is either:
- Triggered as non-interactive step (Phase A)
- Done at your laptop

**What's needed:**
- LLM API integration (Claude/OpenAI/Gemini)
- Context assembly for chat (diff, area files, wave state)
- Conversation persistence
- Chat UI

### Phase C: Agent harness

Chat gains tools. Now it can act.

**Tools:**
- File read/write
- Bash execution
- Glob, grep, search
- Git operations

**Permission system:**
- Mobile approves tool calls
- Structured prompts: "Edit src/foo.rs?" [Approve] [Reject]
- Not raw terminal output

**What's needed:**
- Tool implementations
- Agent loop (prompt → response → tool calls → repeat)
- Permission UI
- Works across Claude/OpenAI/Gemini APIs

This is significant work but gives us a unified agent that works identically across providers, with proper structured events instead of terminal scraping.

## What this replaces

The terminal streaming approach (`concerto-grpc-terminal-streaming.md`) becomes:
- **Deferred**: Not needed for the primary mobile experience
- **Optional escape hatch**: Power users can SSH/Tailscale if they want raw terminal

The lfd registration and auth docs remain relevant—mobile still needs to find and authenticate to lfd.

## Phase A scope (this branch)

First chunk: non-interactive iOS app.

**In scope:**
- iOS target in Swift package
- Wave list view (same groupings as macOS)
- Wave detail with status
- Action buttons: Land, Run Step
- Auth flow (loopflow.studio JWT)
- Connect to remote lfd via HTTP API

**Out of scope:**
- Push notifications (separate item)
- Chat (Phase B)
- Terminal anything

**Done when:**
Conductor can see wave status and land PRs from iPhone.

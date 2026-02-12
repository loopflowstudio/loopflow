---
status: todo
phase: 4
---

# Agent Harness

Chat gains tools. Full agent on mobile.

## Current

Phase 3 chat can discuss code but can't act on it.

## Build

**Tool implementations:**
- File read/write
- Bash execution (sandboxed)
- Glob, grep, search
- Git operations (status, diff, commit)

**Agent loop:**
- Prompt → LLM response → tool calls → repeat
- Streaming execution status
- Interrupt/cancel support

**Permission system:**
- Structured prompts: "Edit src/foo.rs?" [Approve] [Reject]
- Batch approval for related changes
- Never auto-approve destructive actions

**Provider abstraction:**
- Works identically across Claude/OpenAI/Gemini
- Tool schemas translated per provider
- Consistent permission UX regardless of backend

## Why build this

Instead of wrapping three different CLI tools (Claude Code, Codex, Gemini CLI):
- One interaction model
- Structured events (not terminal scraping)
- Full control over UX
- Mobile-native experience

## Tradeoffs

**Lose:**
- Claude Code's polish and ongoing improvements
- Codex/Gemini CLI features we haven't replicated
- The "it just works" of shelling out

**Gain:**
- Unified experience across providers
- Proper mobile UX
- Structured permission system
- No dependency on CLI tool UI stability

## Done when

User can have an agent edit files and run commands from iPhone/iPad, with structured permission prompts.

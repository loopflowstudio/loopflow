# Claude Plan Compatibility: System Prompt Restrictions

Claude's Max plan restricts third-party apps from using `--append-system-prompt` / `--append-system-prompt-file`. Loopflow uses these flags to inject context (docs, diff, directions, step instructions) into Claude sessions. This breaks loopflow for Max plan users.

## Problem

Two paths deliver context to Claude, both blocked:

1. **CLI** (`lf <step>` → `run.rs` → `build_claude_command`): writes context file → `--append-system-prompt-file`
2. **Sessions** (`lfd` → `claude.rs` → `build_claude_session_turn_args`): inline → `--append-system-prompt`

The restriction appears to be on the flags themselves, not content-based (test scripts in `scratch/` were built to binary-search for triggers but the block is likely flag-level on restricted plans).

## What to build

Make context delivery work on plans that block `--append-system-prompt*`, while preserving current behavior for unrestricted plans.

## Approaches

### A: Prefix context to user message (recommended)

Move context from the system prompt into the user message (the `-p` content). Claude Code doesn't restrict user message content.

```rust
// Instead of:
//   --append-system-prompt-file context.md -p "task prompt"
// Emit:
//   -p "<context>\n\ntask prompt"
```

**Pros:**
- Works on all plans — no API-level restriction on user messages
- Minimal code change — context formatting stays the same, just goes to a different slot
- All harnesses already handle task_prompt as user input

**Cons:**
- System prompt vs user message have different caching semantics in the API. System prompts get cached across turns; user messages don't. This increases cost for multi-turn sessions.
- System prompt content has slightly higher priority/attention weight in the model. Moving to user message might slightly degrade instruction following for long contexts.
- On the session path, the system prompt is sent once and persists across turns. Moving to user message means either: (a) sending it every turn (expensive), or (b) sending it only on first turn (then it scrolls out of context in long sessions).

**Mitigations:**
- The session harness already seeds the task_prompt into the first turn only (`should_seed_task_prompt` flag in `claude.rs:96-101`). Context would follow the same pattern — sent on first turn, then the model has it in conversation history.
- For the CLI path, there's only one turn, so no caching difference.

### B: Write to .claude/commands/ or CLAUDE.md

Claude Code reads `CLAUDE.md` and `.claude/commands/` automatically — they become part of Claude Code's own system prompt, potentially bypassing the third-party restriction.

```rust
// Write context to .claude/commands/_lf_context.md
// Then invoke via /lf_context or let CLAUDE.md include it
```

**Pros:**
- Uses Claude Code's native context mechanism
- May bypass the restriction (Claude Code injects its own system prompt regardless)

**Cons:**
- Mutates repo state (CLAUDE.md changes would pollute git status)
- Requires cleanup after each run
- CLAUDE.md has different merging semantics (it's additive with parent dirs)
- `.claude/commands/` requires the user to invoke the command, not automatic
- Fragile — depends on Claude Code's undocumented internal behavior re: plan restrictions

### C: Conditional fallback

Try `--append-system-prompt*` first, detect the restriction error, fall back to approach A.

```rust
// Attempt 1: --append-system-prompt-file context.md -p "task"
// If stderr contains "third-party" / "not allowed":
// Attempt 2: -p "<context>\n\ntask"
```

**Pros:**
- Unrestricted plans keep current behavior (system prompt caching, priority)
- Graceful degradation

**Cons:**
- Adds retry logic and error detection to every launch path
- The restriction error format isn't stable — could break if Claude Code changes error messages
- Doubles startup latency on restricted plans (try, fail, retry)

### D: Context file with tool-read instruction (the reverted approach)

Write context to `.lf/prompts/` and tell the LLM "Read this file first" in the task prompt.

**Cons (why it was reverted):**
- Unreliable — the model may skip reading, hallucinate about it, or partially read
- Adds a tool-use round trip before the agent does any real work
- On models with limited tool access, the file read might fail
- No guarantee the model treats file content with the same priority as system prompt content

## Recommendation

**Approach A** (prefix to user message), with one nuance:

- **CLI path**: Merge `system_prompt` into `task_prompt` unconditionally. There's only one turn.
- **Session path**: The `should_seed_task_prompt` mechanism already exists. Extend it to include the system prompt content in the first turn's user message. Set `system_prompt` to empty so `--append-system-prompt` is never emitted.

The change is surgical:

### Data structures

No new types. `AgentConfig.system_prompt` becomes empty for Claude harness; its content moves to `AgentConfig.task_prompt`.

### Key functions

```rust
// engine/launch.rs — prepare_launch_prompt
// Today: system_prompt = format_context_prompt(&budgeted)
//        task_prompt = format_task_prompt(&budgeted)
// After: system_prompt = String::new()
//        task_prompt = format_context_prompt(&budgeted) + "\n\n" + format_task_prompt(&budgeted)

// engine/agent.rs — build_claude_command, build_claude_session_turn_args
// No changes needed — empty system_prompt already results in no --append-system-prompt flag
// (see to_args line 194: `if !text.trim().is_empty()`)
```

### What about non-Claude harnesses?

Codex, Gemini, and OpenCode use `context_file` through different mechanisms (`-c model_instructions_file=`, `GEMINI_SYSTEM_MD`, `OPENCODE_CONFIG_CONTENT`). These aren't affected by Claude's plan restrictions. The context merge should only apply to the Claude path.

Two options:
1. **Merge in `prepare_launch_prompt` unconditionally** — all harnesses get context in task_prompt. Simplest. Non-Claude harnesses still have context_file set in `run.rs`, so they'd get context twice (once in task_prompt, once via their native mechanism). That's wasteful but not broken.
2. **Merge in the Claude-specific launch path only** — keep `prepare_launch_prompt` as-is, merge in `build_claude_command` and `build_claude_session_turn_args`. More precise but scatters the logic.

Option 1 is simpler if we also stop setting `context_file` for Claude in `run.rs`. Option 2 is more precise. I'd lean toward option 1 with a small guard: only merge when the harness is Claude, or just always merge and let non-Claude harnesses ignore the extra content in task_prompt (they read context_file anyway).

Actually, the cleanest approach: merge in `prepare_launch_prompt`, then have `run.rs` skip the `context_file` write for Claude (since context is already in task_prompt). Non-Claude harnesses still get the context_file write and ignore the extra task_prompt content.

### Constraints

- Don't break non-Claude harnesses
- Don't send context twice on Claude
- Multi-turn sessions: context goes in first turn only (already handled by `should_seed_task_prompt`)

### LOOPFLOW.md trimming

The branch also trims LOOPFLOW.md (removes Area, Chaining, Quality sections). This reduces context size but is orthogonal to the system prompt fix. Can ship independently. The removed content:
- **Area/context orientation** — agents should know this but it's arguably implied by the step and context they receive
- **Chaining** — useful but rarely referenced; step frontmatter handles this
- **Quality** — generic; direction docs cover this better

Keep the trimming as a separate commit.

## Done when

1. `lf debug -c` works on a Claude Max plan account without "third-party" errors
2. `cargo test --all` passes — no regressions on existing agent launch tests
3. Non-Claude harnesses (codex, gemini, opencode) continue to receive context via their native mechanisms
4. Multi-turn sessions via lfd deliver context on the first turn

# How four other tools pass context and handle compaction

Read from source on November 11, 2026 (clones, nothing run). Inspiration
only; no code taken.

| | Base context | After compaction | Provider hooks |
| --- | --- | --- | --- |
| Paseo (`getpaseo/paseo` a886bfb) | System-prompt append (Claude SDK `systemPrompt.append`, Codex `developerInstructions`); harness loads CLAUDE.md | Shows it; re-injects nothing | Status only, off by default, written to global `~/.claude/settings.json`; 500 ms post to daemon |
| T3 Code (`pingdotgg/t3code` 25631c3c) | Claude: system-prompt append. Codex: `additionalContext` on every `turn/start`, under 4,000 bytes | Codex: replays the cached `additionalContext` via `thread/inject_items` (10 s, best-effort). Cursor: re-sends its prefix. Claude: nothing. Beta: server sends `/compact` before the cache expires | One in-process SDK `SubagentStart` |
| HumanLayer (`humanlayer/humanlayer` f9bd7353, rewritten; no `hld`/`thoughts/`) | System-prompt append rebuilt per request: session IDs, task artifact folder, per-user "context shards" on fresh launch only. Auto-memory off; artifacts are the memory | Daemon: records it. Its Claude Code plugin: `PostCompact` clears a flag so the task-folder section rides the next prompt again | In-process SDK PostToolUse only |
| Paradev (`ParadevAI/Paradev` ce98da26, formerly Superset; terminal agents only) | None of its own; task, issue and PR text in the first user message | Nothing | Status hooks merged into global `~/.claude/settings.json` and `~/.codex/hooks.json`, curl to its server |

None delivers base context through a hook. Three put it in the system prompt,
which compaction does not touch; the two that re-inject (T3 for Codex,
HumanLayer's plugin) do it for context that lives in the message stream.

## Codex and the 10,000-byte block (checked November 11)

Codex source at `32234dd` and the installed 0.167.0 binary:
`codex-rs/hooks/src/output_spill.rs` keeps each hook's `additionalContext` to
2,500 tokens, counted as bytes / 4 rounded up, so 10,000 bytes passes whole.
Over that, Codex saves the full text under
`<temp>/hook_outputs/<thread>/<uuid>.txt` and shows the model a head-and-tail
preview ending "Full hook output saved to: <path>". A hook declaration can
raise it with `additionalContextLimit` (tokens). lf's `HOOK_CONTEXT_BYTES` is
10,000 for prefix plus block (`context_block.rs`), the same number; nothing in
lf's code says so. T3 Code's 4,000-byte note is about `additionalContext` on
`turn/start`, a different path.

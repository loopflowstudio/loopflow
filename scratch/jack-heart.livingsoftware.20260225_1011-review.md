# Skill Injection & Living Direction — Review

## What was implemented

Phase 0 and Phase 1 of the Living Waves design:

1. **`living` direction** — new builtin direction (`directions/values/living.md`) establishing "living software" as a product pillar. Agents running with `-d living` evaluate work through this lens.

2. **`engine::skills` module** — `inject_skills(repo, target_dir)` materializes all known steps and directions as `.claude/commands/*.md` files so they appear as slash commands in Claude Code. Handles:
   - Built-in steps (flattened: `scan/scan-report` → `scan-scan-report.md`)
   - Built-in directions (prefixed: `craft` → `direction-craft.md`)
   - Repo-local `.lf/steps/*.md`
   - Repo-local `.lf/directions/*.md` (prefixed)
   - Never overwrites existing user files

3. **Injection wired into all three agent spawn sites**:
   - CLI (`lf/commands/run.rs`) — opt-in via `inject_skills: true` in `.lf/config.yaml`
   - Wave executor (`lfd/executor/wave/mod.rs`) — always on for Claude agents
   - Sessions (`lfd/sessions/mod.rs`) — always on for Claude agents

4. **`inject_skills` config flag** — `Config.inject_skills: bool`, default false. CLI respects it; lfd ignores it (always injects).

5. **Wave plan artifacts** — `wave/living/` with README, YAML config, and wave-memory design doc for Phase 2.

## Key choices

- **Track-and-remove cleanup** over marker comments or subdirectories. Injected file paths are returned by `inject_skills()` and cleaned up via `cleanup_injected_skills()`. Simple, no magic.
- **Opt-in for CLI, always-on for lfd.** CLI users may have their own `.claude/commands/` setup; lfd-managed agents always benefit from full skill visibility.
- **Flat namespace with prefixes.** Steps get their names directly; directions get `direction-` prefix. Namespaced steps get `-` substitution (`scan/scan-report` → `scan-scan-report`).
- **Claude-only for now.** Other backends (Codex, Gemini, OpenCode) don't have a `.claude/commands/` equivalent. System prompt injection for those backends is deferred to later.
- **Cleanup-before-error-propagation pattern.** In all three sites, the agent result is captured, cleanup runs, *then* the result is `?`-propagated. Ensures files are removed even on agent failure.

## How it fits together

```
inject_skills(repo, target) → Vec<PathBuf>   // write .claude/commands/*.md
    ↓ agent runs (slash commands now available)
cleanup_injected_skills(&paths)               // remove written files
```

Three call sites follow the same pattern:
- CLI: synchronous — inject, launch, cleanup, propagate result
- Wave executor: async — inject, `.await` launch, cleanup, propagate result
- Sessions: async — inject on startup (store paths in `SessionRuntime.injected_skills`), cleanup on `stop_session`

## Risks and bottlenecks

- **Orphaned files on crash.** If `lfd` crashes mid-session, injected `.claude/commands/` files remain. They're harmless `.md` files and will be overwritten on next injection, but they linger until manually cleaned or the worktree is removed.
- **Blocking I/O on async tasks.** `inject_skills` does synchronous `fs::write`/`fs::read_dir` inside `tokio::spawn`. Acceptable for the current scale (dozens of files), but would benefit from `tokio::fs` if the file count grows substantially.
- **No global `~/.lf/steps/` injection yet.** Only builtins and repo-local steps are injected. Global user steps and external skill sources (superpowers, rams) are not yet included.

## What's not included

- **Wave memory (Phase 2)** — design doc exists at `wave/living/02-wave-memory.md` but no implementation yet.
- **Surface-adaptive prompts (Phase 3)** — no `Surface` enum or mobile action suggestions.
- **Non-Claude backends** — skill injection is Claude-specific; Codex/Gemini/OpenCode get no new capabilities from this branch.
- **README updates** — no user-facing documentation changes. The `inject_skills` config flag is undocumented beyond the field comment.

## Gate fix applied

Fixed `spawn_harness_startup` in sessions to thread `repo_root` separately from `cwd`. Previously used `cwd` as both the repo root (for reading `.lf/steps/`) and target (for writing `.claude/commands/`). When `cwd != repo_root`, repo-local steps would not be discovered.

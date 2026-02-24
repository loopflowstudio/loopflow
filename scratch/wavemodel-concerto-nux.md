# Concerto NUX (Design-First Onboarding)

Reshape Concerto's new user experience to orient around design-first wave creation. Ship what we can now; full interactive sessions arrive with agentapi.

## Prerequisite: Swift WaveSchema cleanup

Phase 02 removed the `GET /wave/schemas` endpoint and all Rust-side schema code. The Swift client still references `WaveSchema` in `LocalWaveService.swift`, `WaveSidebar.swift`, `RepoState.swift`, and `WaveServiceProtocol.swift`. These calls return `[]` (error path) so the app doesn't crash, but the code is dead. Clean this up first — these are the same files this phase modifies.

## What changes now

**`StartWaveView` → design-first framing.** Currently "Start a wave" with a name text field.
- "What do you want to build?" stays — it's the right question
- Text field becomes a prompt/description, not a wave name
- Submit launches `lf design` with the user's input as clipboard context (`-c`)
- Wave name derived from design conversation, not typed upfront
- Until agentapi ships, this opens a terminal session. After, it becomes an inline interactive session.

**`WaveSidebar` — hide orphan worktrees by default.** Worktrees section is implementation detail, not for new users. Collapse behind disclosure or remove from default view. Accessible via diagnostics or settings toggle.

**`WaveDetailPanel` — surface wave content.** Currently purely operational. Add:
- Vision as subtitle under wave name
- Goals visible when idle — what you're working toward
- Risks visible when reviewing
- Roadmap progress — read `##-*.md` files alongside the README to show which items are done and what's next
- Read README from disk via `{repo}/wave/{name}/README.md`. Parse sections client-side.
- Read roadmap from disk via `{repo}/wave/{name}/##-*.md`. Parse status from file content or naming.
- Note from Phase 01: scope boundaries appear in different locations across waves ("Not here" under Vision, "Security boundary" at the end, etc.). Parser should match `## Vision`, `## Goals`, `## Risks`, `## Metrics` as the four README sections and treat everything else as supplementary.

**Empty state emphasizes design.** "No waves yet" + "Start designing" button instead of "Create Wave".

## What waits for agentapi

- Inline interactive `lf design` session in Concerto (needs agentapi phases 01-03)
- Real-time README section population during design conversation
- Wave creation confirmation UI after design completes

## What Phase 02 validated

- `wave_config.rs` reads `wave/<name>/<name>.yaml` cleanly. The pattern (read from disk, return `None` for missing) is a reference for how Concerto should read `wave/<name>/README.md`.
- Wave YAML on disk works as the source of truth. No schema abstraction needed. Concerto can read wave content directly from the filesystem — no need for a new API endpoint to serve README content.
- Directory name is canonical (`wave/<name>/`). Concerto can derive the wave name from the directory path without parsing YAML.

## Follow-up: chat sessions must be wave-aware

Current behavior allows chat sessions to start without an explicit wave/run context, which means the harness can lose the intended worktree/repo scope.

Add a follow-up requirement:
- Concerto must create agent sessions with the selected `wave_run_id`
- Session config must include the active wave worktree as `cwd`
- lfd should resolve/validate that `cwd` against the run/worktree mapping
- Chat UI should display the resolved wave/run/worktree context for transparency

## Files touched

| File | Change |
|------|--------|
| `swift/Concerto/Views/StartWaveView.swift` | Reframe as design entry, launch `lf design -c` |
| `swift/Concerto/Views/WaveSidebar.swift` | Hide orphan worktrees, update empty state |
| `swift/Concerto/Views/WaveDetailPanel.swift` | Add wave content section |
| `swift/LoopflowCore/Models/WaveViewModel.swift` | Add optional content fields |
| New: wave content parsing utility | Parse README.md sections from disk |

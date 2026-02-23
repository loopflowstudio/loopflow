# Concerto NUX (Design-First Onboarding)

Reshape Concerto's new user experience to orient around design-first wave creation. Ship what we can now; full interactive sessions arrive with agentapi.

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
- Roadmap progress — which phases done, what's next
- Read from disk via `{repo}/wave/{name}/README.md`. Parse sections client-side.
- Note from Phase 01: scope boundaries appear in different locations across waves ("Not here" under Vision, "Security boundary" at the end, etc.). Parser should match `## Vision`, `## Goals`, `## Risks`, `## Metrics`, `## Roadmap` as primary sections and treat everything else as supplementary.

**Empty state emphasizes design.** "No waves yet" + "Start designing" button instead of "Create Wave".

## What waits for agentapi

- Inline interactive `lf design` session in Concerto (needs agentapi phases 01-03)
- Real-time README section population during design conversation
- Wave creation confirmation UI after design completes

## Files touched

| File | Change |
|------|--------|
| `swift/Concerto/Views/StartWaveView.swift` | Reframe as design entry, launch `lf design -c` |
| `swift/Concerto/Views/WaveSidebar.swift` | Hide orphan worktrees, update empty state |
| `swift/Concerto/Views/WaveDetailPanel.swift` | Add wave content section |
| `swift/LoopflowCore/Models/WaveViewModel.swift` | Add optional content fields |
| New: wave content parsing utility | Parse README.md sections from disk |

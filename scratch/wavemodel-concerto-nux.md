# Concerto NUX: Design-First Onboarding

## Problem

New users opening Concerto see "Start a wave" with a name field—a configuration form for a concept they don't yet understand. The sidebar shows orphan worktrees (implementation details) and schema-based wave templates (dead code since Phase 02). The detail panel is purely operational: status, output, commits. Nothing shows *what* a wave is about—its vision, goals, or progress.

The result: new users bounce. They don't know what to type, what a wave name means, or why they'd create one. The app orients around operations instead of intent.

Who benefits: every new user, plus existing users who want to see wave content without reading markdown files on disk.

Why now: Phase 01 standardized wave READMEs (Vision/Goals/Risks/Metrics). Phase 02 removed schema abstractions and validated filesystem-based wave config. The content exists and the reading patterns are proven. Concerto just doesn't surface any of it.

## Approach

Three changes ship together, preceded by dead code cleanup:

### Phase 0: WaveSchema Cleanup

Remove all `WaveSchema` references from Swift. Phase 02 deleted the Rust-side `GET /wave/schemas` endpoint. The Swift client still declares the protocol method, stores schemas in `RepoState`, renders them in `WaveSidebar`, and offers "Instantiate all" in the create menu. All dead code, all in files this phase modifies.

**Files:**
- `WaveServiceProtocol.swift` — remove `listWaveSchemas` method
- `WaveSchema.swift` — delete entire file
- `RepoState.swift` — remove `waveSchemas` field, `instantiateSchema()`, schema loading in refresh
- `WaveSidebar.swift` — remove `pendingSchemas`, schema menu items, instantiation logic
- `LocalWaveService.swift` / `MockWaveService.swift` — remove `listWaveSchemas` implementations

### Phase 1: StartWaveView → Design Entry

Replace the wave name form with a design prompt launcher.

**Current flow:** Type wave name → create wave → configure later.

**New flow:** Describe what you want to build → launch `lf design -c "<description>"` in terminal → design session creates the wave.

The text field stays but changes meaning: from "wave name" to "design prompt." Placeholder text: "Describe what you want to build..." Submit button text: "Start designing."

On submit, use `TerminalLauncher.launchTerminal(_:at:command:)` to run `lf design -c "<user input>"` at the repo root. The design step is interactive—it asks follow-up questions, walks through components, and either produces a `scratch/` design doc or a full `wave/` directory with README and roadmap.

The terminal launch is intentional friction. The user typed their intent in Concerto; the design conversation happens in a terminal where `lf design` can use the full coding agent (Claude Code). When agentapi ships, this becomes an embedded interactive session using the existing `InteractiveSessionView` + `GhosttyTerminalView` infrastructure.

**Shell-escape the user input.** The description goes into a shell command. Single quotes in the input must be escaped. Use the existing `escapeShellSingleQuotes` helper in TerminalLauncher.

**No wave creation on submit.** The old flow called `repoState.createWave(name:)`. The new flow delegates wave creation to `lf design`. Concerto picks up the new wave on its next refresh cycle.

### Phase 2: WaveDetailPanel → Surface Wave Content

Add a content section to the detail panel that reads and displays wave README sections and roadmap progress.

**New utility: `WaveContentParser`** — reads `{repo}/wave/{name}/README.md` and `{repo}/wave/{name}/##-*.md` files from disk.

README parsing is a line-by-line state machine:
1. Scan for `## Vision`, `## Goals`, `## Risks`, `## Metrics` headers
2. Collect lines between matching headers into named sections
3. Everything not under one of these four headers is supplementary (ignored for display)
4. Subsections within a section (e.g., "### Not here" under Vision) stay with their parent

Return a `WaveContent` struct:
```swift
public struct WaveContent: Sendable, Equatable {
    public var vision: String?     // Text under ## Vision
    public var goals: String?      // Text under ## Goals
    public var risks: String?      // Text under ## Risks
    public var metrics: String?    // Text under ## Metrics
    public var roadmapItems: [RoadmapItem]
}

public struct RoadmapItem: Sendable, Identifiable, Equatable {
    public var id: String          // Filename stem (e.g., "01-codex-e2e")
    public var number: Int         // Parsed from prefix
    public var title: String       // First heading or filename
    public var isShipped: Bool     // Has "## Shipped" section
}
```

Roadmap parsing:
1. Glob `{repo}/wave/{name}/[0-9][0-9]-*.md`
2. For each file, read the first `# ` heading as title
3. Check for `## Shipped` section to determine completion status
4. Sort by number prefix

**Display in WaveDetailPanel:**
- Vision appears as a subtitle below the wave name in the header area. One or two lines, muted secondary text. Truncate with ellipsis if longer.
- Goals section visible when the wave is idle—this is what you're working toward. Rendered as a compact list.
- Risks section visible when reviewing—what to watch for. Shown alongside the review output.
- Roadmap progress as a compact list with checkmarks for shipped items and circles for pending. Shows item number and title.

**Content loading:** Read from disk when the wave is selected (detail panel appears). Cache in `WaveViewModel` as `content: WaveContent?`. Refresh when the panel regains focus or when a step completes (wave status changes). Don't watch the filesystem continuously—read on demand.

**Add content fields to WaveViewModel:**
```swift
public var content: WaveContent?
```

Content loading lives in `RepoState` alongside existing wave data fetching. New method `loadWaveContent(for:)` reads from `{currentRepo}/wave/{waveName}/` using `WaveContentParser`.

### Phase 3: WaveSidebar → Clean Default View

**Hide orphan worktrees by default.** Wrap the worktrees section in a `DisclosureGroup` that defaults to collapsed. Users who need worktree visibility can expand it. State persists via `@AppStorage`.

**Update empty state.** Change from "No waves yet" + "Create Wave" button to "No waves yet" + "Start designing" button. The button navigates to StartWaveView (or triggers the same design flow inline).

**Simplify create menu.** Remove schema-related menu items (Phase 0). The create button becomes a single action that opens StartWaveView, not a menu with options.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Embed design session inline using existing GhosttyTerminalView | More integrated experience, no context switch | Requires a wave ID for InteractiveSession, but design *creates* the wave. Would need a "pre-wave" session concept. Ship the terminal launch now; agentapi enables the inline version. |
| Add API endpoint to serve README content | Cleaner separation, works for remote repos | Wave item explicitly says no API needed. Filesystem reads follow the `wave_config.rs` pattern. Remote repo support can use the existing remote file access when needed. |
| Full markdown parser library for README | More robust parsing | Overkill. Wave READMEs follow a known four-section convention. A state machine matching `## Section` headers is sufficient and has zero dependencies. |
| Remove worktrees section entirely | Cleaner sidebar | Too aggressive. Power users use worktrees for debugging. Disclosure group gives both audiences what they want. |
| Keep wave name field alongside design prompt | Backward compatible | Two fields for one action is confusing. Wave names come from design conversations. Users who want manual naming can use `lfq create` from the terminal. |

## Key decisions

1. **External terminal for design, not embedded.** The existing `InteractiveSession` requires a `waveId`, but `lf design` creates the wave. Launching in a terminal is the right interim step. Wave principle: "Ship what we can now; full interactive sessions arrive with agentapi."

2. **Filesystem reads, no API.** Phase 02 validated: "`wave_config.rs` reads `wave/<name>/<name>.yaml` cleanly. The pattern is a reference for how Concerto should read `wave/<name>/README.md`." Follow the same pattern—read from disk, return nil for missing.

3. **State-machine parser, not regex or library.** The README convention is known: four section headers. A line-by-line scan matching `## Vision`, `## Goals`, `## Risks`, `## Metrics` is correct, simple, and matches the risk callout: "Parser should match `## Vision`, `## Goals`, `## Risks`, `## Metrics` as the four README sections and treat everything else as supplementary."

4. **Roadmap status from `## Shipped` section.** Roadmap files use a "## Shipped" section to mark completion. This matches the existing convention (visible in `agentapi/01-codex-e2e.md`, `agentapi/02-claude-adapter.md`, etc.). No separate status field or naming convention needed.

5. **Content cached on WaveViewModel, loaded on demand.** No filesystem watcher. Read when the detail panel appears and when wave status changes. This matches the existing pattern where `WaveViewModel` holds operational data refreshed by `RepoState`.

6. **Worktrees behind disclosure, not removed.** Wave sidebar goal: "hide orphan worktrees by default." Disclosure group with `@AppStorage` persistence gives new users a clean view and power users their worktree list.

## Scope

**In scope:**
- Remove dead `WaveSchema` code from Swift (prerequisite, same files)
- StartWaveView redesign: design prompt → terminal launch
- WaveContentParser: read README sections and roadmap from disk
- WaveDetailPanel: show vision, goals, risks, roadmap
- WaveSidebar: hide worktrees, update empty state, simplify create
- WaveViewModel: add `content: WaveContent?` field

**Out of scope:**
- Embedded interactive design session (agentapi dependency)
- Real-time README population during design conversation
- Wave creation confirmation UI after design completes
- Remote repo wave content reading (future, separate concern)
- Filesystem watching for live content updates
- Database schema changes for wave content (stays in markdown per wave README)
- Enforced validation of section presence (convention first per wave README)

## Files touched

| File | Change |
|------|--------|
| `swift/LoopflowCore/Models/WaveSchema.swift` | Delete |
| `swift/LoopflowCore/Services/WaveServiceProtocol.swift` | Remove `listWaveSchemas` |
| `swift/LoopflowCore/Services/LocalWaveService.swift` | Remove schema implementation |
| `swift/LoopflowCore/Services/MockWaveService.swift` | Remove schema implementation |
| `swift/Concerto/State/RepoState.swift` | Remove schema fields, add `loadWaveContent` |
| `swift/Concerto/Views/StartWaveView.swift` | Redesign as design prompt launcher |
| `swift/Concerto/Views/WaveSidebar.swift` | Hide worktrees, update empty state, remove schemas |
| `swift/Concerto/Views/WaveDetailPanel.swift` | Add wave content section |
| `swift/LoopflowCore/Models/WaveViewModel.swift` | Add `content: WaveContent?` |
| New: `swift/LoopflowCore/Models/WaveContent.swift` | `WaveContent`, `RoadmapItem` structs |
| New: `swift/LoopflowCore/Services/WaveContentParser.swift` | README + roadmap file parser |

## Done when

```bash
# Swift package builds clean
swift build --package-path swift

# Concerto builds and tests pass
cd swift && xcodegen generate && xcodebuild test \
  -project LoopflowSwift.xcodeproj -scheme Concerto \
  -destination 'platform=macOS'

# No WaveSchema references remain
grep -r "WaveSchema" swift/ && echo "FAIL: dead code remains" || echo "PASS"

# WaveContentParser correctly parses a wave README
swift test --package-path swift --filter WaveContentParser
```

Observable: Open Concerto with a repo containing waves. The sidebar shows waves without a worktrees section. Select a wave—vision appears under the name, goals and roadmap are visible. Click "Start designing" from the empty state or the sidebar—a terminal opens with `lf design`.

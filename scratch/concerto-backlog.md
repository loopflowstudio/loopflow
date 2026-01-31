# Concerto Backlog Restructure

Transform `roadmap/concerto-next/` into a pure backlog. Move design docs to `reports/`, personas to `.lf/directions/`, and UX test scripts to `swift/`.

## What to build

- `roadmap/concerto/` — pure backlog of pickable work items
- `reports/concerto/` — design reference (current 00-09 docs)
- `.lf/directions/` — personas as directions
- `swift/ConcertoTests/UXExperiments/` — executable test scripts

## Current state → New locations

| Current | New | Why |
|---------|-----|-----|
| `roadmap/concerto-next/00-09*.md` | `reports/concerto/` | Reference docs, not backlog |
| `roadmap/concerto-next/07-ux-experiments.md` | Split: personas → `.lf/`, scripts → `swift/` | |
| (new) | `roadmap/concerto/` | Pickable work items |

## Backlog structure

```
roadmap/concerto/
├── README.md
├── status-dashboard.md
├── connect-flow.md
├── continue-button.md
├── land-flow.md
├── area-picker.md
├── step-runner.md
├── wave-creation.md
└── local-notifications.md
```

Each item:

```yaml
---
status: todo | in-progress | done
phase: 1
verifies: [E2, E5]  # which UX experiments test this
---
```

```markdown
# Status Dashboard

Wave list grouped by NEEDS YOU / RUNNING / READY TO LAND.

## Current
WaveSidebar.swift shows flat list with status badges.

## Build
- Group waves by status category
- Section headers with counts
- Priority ordering within groups

## Done when
E2 passes: "Status clear in < 5 seconds"
```

## Personas as directions

Personas become `.lf/directions/` files—a lens that applies to any step and any area. See PROMPT_STYLE.md § Orthogonality.

```markdown
# .lf/directions/conductor.md

Managing multiple parallel workstreams. Checking in, not diving deep.

- Can I see what needs attention without drilling in?
- Is urgency visually obvious?
- How many clicks from "I see a problem" to "I'm acting on it"?
- Would I trust this to surface the right thing while I'm away?
- Do routine actions feel like single actions or workflows?
- Am I confirming things I already decided?

Red flags:
- Flat lists with no hierarchy
- Status requires interaction to reveal
- Confirmation dialogs for routine operations
```

```markdown
# .lf/directions/improviser.md

Exploring unfamiliar territory. Doesn't know the right approach yet.

- How fast from intent to action?
- Am I configuring things I don't care about yet?
- Can I do one thing without committing to a sequence?
- Does this feel like a workshop or a form?
- If I change my mind, is that cheap?
- Am I exploring or filling out paperwork?

Red flags:
- Setup wizards before first action
- Required fields for optional concepts
- Changing course requires starting over
```

```markdown
# .lf/directions/returner.md

Was away. Needs to catch up and triage.

- Can I tell what happened while I was gone?
- Is "needs me" vs "fine" instant to distinguish?
- Do I have to click into each item to understand state?
- Is there a summary or must I reconstruct context?
- Can I triage N items in a few minutes?

Red flags:
- No history/timeline visible
- State only visible after navigation
- Context lost between sessions
```

## UX experiments as Swift tests

Experiments become executable tests in `swift/ConcertoTests/UXExperiments/`. They drive the app through scenarios and can capture screenshots, measure timing, log friction.

```swift
// swift/ConcertoTests/UXExperiments/E2_MorningCheckin.swift

import XCTest
@testable import Concerto

/// Persona: Returner
/// Target: Status clear in < 5 seconds, each wave handled in < 2 min
final class E2_MorningCheckin: XCTestCase {

    /// Setup: 5 waves in various states
    func setupScenario() async throws -> RepoState {
        // Create test waves:
        // - 2 in "needs you" (waiting for interactive)
        // - 2 running
        // - 1 ready to land
    }

    func test_statusVisibleQuickly() async throws {
        let state = try await setupScenario()
        let start = Date()

        // Simulate: open app, look at dashboard
        let dashboard = DashboardView(state: state)

        // Can I tell what needs attention?
        let needsYou = dashboard.wavesNeedingAttention
        XCTAssertEqual(needsYou.count, 2)

        let elapsed = Date().timeIntervalSince(start)
        XCTAssertLessThan(elapsed, 5.0, "Status should be clear in < 5 seconds")
    }

    func test_canHandleWaveQuickly() async throws {
        // Measure: connect → review → continue for one wave
    }

    func test_canLandReadyPR() async throws {
        // Measure: find ready wave → land
    }
}
```

For more visual testing (screenshots, simulated user flow):

```swift
// swift/ConcertoTests/UXExperiments/E2_MorningCheckin_Visual.swift

final class E2_MorningCheckin_Visual: XCTestCase {

    func test_captureUserFlow() async throws {
        let state = try await setupScenario()

        // Capture screenshots at each step
        let screenshots = CaptureService()

        screenshots.capture("01-open-app", view: RepoWindow(state: state))

        // Simulate: user scans for "needs you"
        screenshots.capture("02-see-needs-you", view: ...)

        // Simulate: user clicks connect
        screenshots.capture("03-connect", view: ...)

        // Export to scratch/ for review
        try screenshots.exportAll(to: "scratch/E2-captures/")
    }
}
```

## Key functions

```python
# Backlog item loading
@dataclass
class BacklogItem:
    name: str
    status: str  # todo, in-progress, done
    phase: int
    verifies: list[str]  # experiment IDs
    content: str

def load_backlog(path: Path) -> list[BacklogItem]:
    """Load all items from roadmap/concerto/."""
    ...

def pick_next_item(backlog: list[BacklogItem]) -> BacklogItem:
    """Pick highest priority todo item."""
    ...
```

## Constraints

- Backlog items must reference at least one experiment
- Experiments are executable (Swift tests), not just documentation
- Personas are directions that can be passed to `lf` commands
- Design docs in `reports/` are reference, not source of truth for work

## Done when

```bash
# Structure
ls roadmap/concerto/*.md           # backlog items
ls reports/concerto/*.md           # design reference
ls .lf/directions/conductor.md     # persona directions
ls swift/ConcertoTests/UXExperiments/  # test scripts

# Backlog works with ingest
lf ingest  # picks from roadmap/concerto/

# Personas work with lf
lf review --direction conductor    # shapes review through persona lens

# Tests run
swift test --filter UXExperiments  # experiments execute
```

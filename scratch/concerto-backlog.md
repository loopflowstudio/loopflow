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

Personas become `.lf/directions/` files. They shape how an agent approaches Concerto work—same as product-engineer or designer.

```markdown
# .lf/directions/conductor.md

Build for users who have multiple waves running and check in periodically.

## What they want
- Glanceable status
- Quick connect to interactive steps
- Minimal friction to land

## Quality signals
- Can I tell what needs attention in < 5 seconds?
- How many clicks to connect?
- Is wave state obvious?

## Anti-patterns
- Burying "needs attention" in a flat list
- Requiring navigation to see status
- Modal confirmations for routine actions
```

```markdown
# .lf/directions/improviser.md

Build for users starting fresh on a problem, exploring before committing to a flow.

## What they want
- Quick wave creation
- Easy step running
- Low commitment

## Quality signals
- How fast from "I have an idea" to "step is running"?
- Can I iterate without ceremony?
- Does the UI get out of my way?

## Anti-patterns
- Wizards and setup flows
- Requiring full wave config before running a step
- Making exploration feel like commitment
```

```markdown
# .lf/directions/returner.md

Build for users who were away and need to catch up on wave status.

## What they want
- See what happened while away
- Quick triage: what needs me vs what's fine
- Decide next actions fast

## Quality signals
- Can I catch up on 5 waves in 2 minutes?
- Is history/progress visible?
- Are notifications useful, not noisy?

## Anti-patterns
- Requiring click-through to see status
- Losing context when away
- Notification spam
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

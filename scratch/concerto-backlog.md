# Concerto Backlog Restructure

Transform `roadmap/concerto-next/` from narrative design docs into a pickable backlog with simulated UX testing workflow.

## What to build

A backlog structure where each item is a discrete piece of work that can be ingested, implemented, and verified via persona-based UX simulation.

## Current state

```
roadmap/concerto-next/
├── 00-overview.md      # narrative
├── 01-platform.md      # narrative
├── ...
├── 07-ux-experiments.md  # personas + test scripts (useful!)
├── 09-phasing.md       # done criteria
└── README.md
```

The design docs are valuable reference but not pickable work items. The UX experiments file has personas and test scripts—these become the verification mechanism.

## New structure

```
roadmap/concerto/
├── README.md           # overview, points to docs/ and backlog/
├── docs/               # design reference (current 00-09 files)
│   ├── overview.md
│   ├── conduct-ux.md
│   ├── improvise-ux.md
│   └── ...
├── personas/           # who we're building for
│   ├── conductor.md
│   ├── improviser.md
│   └── returner.md
├── experiments/        # UX test scripts
│   ├── E1-quick-debug.md
│   ├── E2-morning-checkin.md
│   ├── E3-ship-feature.md
│   ├── E4-start-project.md
│   └── E5-multi-wave.md
└── backlog/            # pickable work items
    ├── status-dashboard.md
    ├── connect-flow.md
    ├── continue-button.md
    ├── land-flow.md
    ├── area-picker.md
    ├── step-runner.md
    ├── wave-creation.md
    ├── local-notifications.md
    └── ...
```

## Backlog item format

Each item in `backlog/` follows this structure:

```yaml
---
status: todo | in-progress | done
phase: 1 | 2 | 3 | 4
tests: [E1, E3]  # which experiments verify this
personas: [conductor, returner]  # who benefits
---
```

```markdown
# Status Dashboard

Wave list grouped by NEEDS YOU / RUNNING / READY TO LAND.

## What exists
- WaveSidebar.swift shows flat list
- WaveRow.swift displays individual waves

## What to build
- Group waves by status category
- Section headers: "NEEDS YOU", "RUNNING", "READY TO LAND"
- Priority ordering within groups

## Done when
Experiment E2 (morning check-in): "Status clear in < 5 seconds"
```

## Simulated UX testing flow

The experiments become a verification mechanism. After implementing a feature:

```bash
lf ux-test E2  # runs persona simulation against the app
```

The step:
1. Reads the experiment script (E2-morning-checkin.md)
2. Reads relevant persona (returner.md)
3. Uses Claude to simulate the persona walking through the experiment
4. Reports friction points, missing affordances, timing estimates

This creates a feedback loop: implement → simulate → identify gaps → implement.

## Persona files

Extract from 07-ux-experiments.md into standalone files:

```markdown
# roadmap/concerto/personas/conductor.md

**The Conductor**

Has multiple waves running. Checks in periodically. Connects for interactive steps, reviews output, lands PRs.

## Wants
- Glanceable status
- Quick connect
- Minimal friction to land

## Typical session
- Open app, scan wave list
- Handle anything in "NEEDS YOU"
- Land anything in "READY TO LAND"
- Check "RUNNING" waves are healthy

## Pain points
- Slow to see what needs attention
- Too many clicks to connect
- Unclear wave state
```

## Experiment files

```markdown
# roadmap/concerto/experiments/E2-morning-checkin.md
---
persona: returner
features: [status-dashboard, connect-flow, land-flow]
target: "Status clear in < 5 seconds, each wave handled in < 2 min"
---

# E2: Morning check-in

## Script
1. Open Concerto
2. See which waves need attention
3. For each waiting wave: connect, review, continue or fix
4. Land any ready PRs
5. Check running waves are healthy

## Success criteria
- Status clear in < 5 seconds
- Each wave handled in < 2 min

## Friction signals
- User hesitates (where?)
- User clicks wrong thing
- User asks "how do I..."
- User can't find information
```

## Backlog items (Phase 1)

From 09-phasing.md, Phase 1 deliverables become these items:

| Item | Experiments | Personas |
|------|-------------|----------|
| status-dashboard | E2, E5 | conductor, returner |
| connect-flow | E2, E3 | conductor, returner |
| continue-button | E3 | conductor |
| land-flow | E2, E3 | conductor, returner |
| area-picker | E1, E4 | improviser |
| step-runner | E1, E4 | improviser |
| wave-creation | E4 | improviser |
| local-notifications | E3 | conductor |

## Key functions

```python
# New step: ux-test
# .lf/steps/ux-test.md
def run_ux_test(experiment: str) -> UXTestResult:
    """Simulate persona walking through experiment."""
    ...

@dataclass
class UXTestResult:
    experiment: str
    persona: str
    friction_points: list[str]
    missing_affordances: list[str]
    estimated_time: str
    verdict: str  # "PASS" | "FRICTION" | "BLOCKED"
```

## Constraints

- Keep design docs as reference—don't delete them
- Each backlog item must link to at least one experiment
- Experiments are the acceptance criteria
- Persona simulations use the actual app state (screenshot or DOM inspection where possible)

## Done when

```bash
# Rename works
ls roadmap/concerto/  # exists
ls roadmap/concerto-next/  # gone

# Structure correct
ls roadmap/concerto/backlog/  # has items
ls roadmap/concerto/experiments/  # has E1-E5
ls roadmap/concerto/personas/  # has 3 personas

# Can ingest
lf ingest  # picks an item from backlog/
```

# spark-tune: LLM-Assisted UX Iteration for Maestro

**What to build**: Infrastructure for LLMs to see, understand, and iterate on the Maestro app UX—including screenshots, simulated user research, and minimal automated UI testing.

## Vision

The user wants a "debug loop" for UX iteration:

> "(1) we make sure to build the infra necessary for LLMs like claude code and codex to easily 'use' and 'see' our app to understand the ux issues with it
> (2) we define a pipeline that involves doing:
>   (a) looking at the state of the app and the stated vision and identifying the biggest gaps
>   (b) simulating those 'users' and conducting user research
>   (c) brainstorming and designing new UXes to test
>   (d) building those etc..."

## Data Structures

```python
@dataclass
class UISnapshot:
    """Captured state of the Maestro app for LLM analysis."""
    screenshot_path: Path          # PNG screenshot
    view_hierarchy: dict           # Accessibility tree (optional)
    app_state: dict               # Serialized AppState
    timestamp: datetime

@dataclass
class UserPersona:
    """Simulated user for UX research."""
    name: str                      # e.g., "power-user", "newcomer"
    description: str               # Goals, pain points, experience level
    tasks: list[str]              # What they're trying to accomplish

@dataclass
class UXQuestion:
    """Research question to test with simulated users."""
    question: str                  # "Can the user find how to create a worktree?"
    persona: str                   # Which persona to test with
    success_criteria: str          # What would success look like?
```

## Key Functions

### Screenshot Capture

```python
def capture_maestro_screenshot(window_title: str = "Maestro") -> Path:
    """Take screenshot of Maestro window, save to .design/screenshots/."""

def capture_with_state(app_state_json: Path) -> UISnapshot:
    """Capture screenshot + dump current AppState for context."""
```

Implementation: Use macOS `screencapture` CLI with window selection. The Maestro app can export its state via a debug endpoint or file dump.

### State Export (Swift side)

```swift
extension AppState {
    func exportDebugState() -> String {
        """JSON dump of current state for LLM analysis."""
    }
}

// Add menu item: Debug > Export State (⌘⇧D)
// Writes to .design/state.json in repo root
```

### UX Analysis Pipeline (`.lf/ux-*.lf`)

```
.lf/
  ux-audit.lf       # Analyze screenshots + vision → identify gaps
  ux-research.lf    # Simulate users, run through tasks
  ux-design.lf      # Propose UI improvements based on findings
  ux-build.lf       # Implement proposed changes
```

## Pipeline Design

### 1. `lf ux-audit`

Input:
- Screenshots in `.design/screenshots/`
- `docs/vision.md` and `docs/maestro.md`
- Current state export

Output: `.design/ux-audit.md` containing:
- List of gaps between vision and reality
- Prioritized UX issues
- User research questions to investigate

### 2. `lf ux-research`

Input:
- `.design/ux-audit.md` (issues and questions)
- Persona definitions in `.design/personas/`
- Screenshots

Output: `.design/ux-research.md` containing:
- Simulated user walkthrough notes
- Pain points discovered
- Severity ratings

### 3. `lf ux-design`

Input:
- `.design/ux-research.md`
- Current SwiftUI code

Output: `.design/ux-proposal.md` containing:
- Proposed UI changes (with sketches in prose)
- SwiftUI code snippets showing approach
- Before/after comparisons

### 4. `lf ux-build`

Input:
- `.design/ux-proposal.md`
- Maestro codebase

Output: Working code implementing the proposal

## UI Testing (Pre-commit Hook)

Minimal smoke tests that catch obvious regressions:

```swift
// MaestroTests/UISnapshotTests.swift

@Test("App launches without crash")
func launchTest() {
    let app = AppState()
    #expect(app.worktrees.isEmpty)  // Initial state
}

@Test("PromptLauncher renders with all controls")
func promptLauncherControls() {
    let state = AppState()
    let view = PromptLauncher(appState: state)
    // SwiftUI Preview rendering test
}
```

For actual visual regression testing, use reference screenshots:

```bash
# .lf/hooks/pre-commit
#!/bin/bash
# Capture current screenshots and compare to reference
maestro-test screenshot --compare .design/reference/
```

## Implementation Plan

### Phase 1: Screenshot Infrastructure

1. Add `capture_screenshot.py` CLI tool:
   - Uses `screencapture -l <window_id>` for window-specific capture
   - Finds Maestro window via `osascript` or window title
   - Saves to `.design/screenshots/<timestamp>.png`

2. Add Debug menu to Maestro app:
   - `Export State` (⌘⇧D) → writes JSON to `.design/state.json`
   - `Capture Screenshot` → triggers capture + state export together

### Phase 2: UX Prompts

Create the four `.lf/ux-*.lf` files with prompts that:
- Load screenshots as context (via `-x .design/screenshots/`)
- Reference vision docs for gap analysis
- Output structured findings to `.design/`

### Phase 3: Automated Testing

1. Add SwiftUI preview tests using Swift Testing framework
2. Create reference screenshots for regression comparison
3. Wire up pre-commit hook for UI smoke tests

## Constraints

- **Screenshots are primary context.** LLMs can see and reason about screenshots. The infrastructure must make it trivial to capture and include them.

- **State export must be opt-in.** Debug menus, not automatic. Users shouldn't wonder why JSON files appear.

- **UI tests must be fast.** Pre-commit hooks need to finish in <5 seconds. Visual regression is optional/manual.

- **Personas are text files.** No complex persona engine. Just markdown files in `.design/personas/` that prompts can read.

## Done When

1. Running `screencapture-maestro` produces a PNG in `.design/screenshots/`
2. Maestro Debug > Export State writes `.design/state.json`
3. `lf ux-audit` reads screenshots and outputs gap analysis
4. `swift test` passes with UI smoke tests
5. Pre-commit hook runs UI tests automatically

Verification:
```bash
# Capture screenshot
screencapture-maestro

# Export state (manual in app)
# Debug > Export State

# Run audit
lf ux-audit

# Check tests
cd Maestro && swift test

# Verify hook
git commit --dry-run  # should run UI tests
```

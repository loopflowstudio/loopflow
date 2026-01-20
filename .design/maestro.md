# Maestro Screenshot Pipeline

Automated screenshot generation for documentation. Point Maestro at a real demo repo, capture the actual UI, propagate to docs.

## What to build

A script that:
1. Sets up a demo repo with realistic worktrees/branches
2. Launches Maestro pointing at that repo
3. Captures screenshots of specific views
4. Copies images to `docs/`

## Current state

- `CaptureService.swift` captures the key window to `/tmp/maestro-<timestamp>.png`
- VHS `.tape` files generate CLI demo GIFs
- No automated Maestro screenshot workflow exists

## Approach: Real repo, real data

Use `loopflow-demos` repo (or a local clone) with actual git state:
- Real worktrees created via `wt create`
- Real branches with commits
- Real PRs (optional, or mocked PR URLs)
- Real `.lf/config.yaml` and prompts

Maestro loads this like any other repo—no special "demo mode" needed. The screenshots show exactly what users see.

## Data structures

```python
# scripts/screenshot_manifest.py

@dataclass
class Screenshot:
    name: str                    # Output filename without extension
    view: str                    # "main", "prompt", "worktree-detail"
    window_size: tuple[int, int] # (width, height)
    setup: str | None            # Optional setup command before capture

@dataclass
class Manifest:
    repo_url: str                # Demo repo to clone/use
    screenshots: list[Screenshot]
```

```yaml
# scripts/screenshots.yaml
repo: https://github.com/loopflowstudio/loopflow-demos
local: ~/src/loopflow-demos  # Clone here if not present

screenshots:
  # Main window - shows sidebar + prompt launcher
  - name: maestro-main
    window_size: [1200, 800]
    requires:
      - worktrees: 2+
      - some_ahead_of_main: true

  # Worktree detail - shows selected worktree with actions
  - name: maestro-worktree
    window_size: [1200, 800]
    select_worktree: first_with_commits
    requires:
      - worktree_with_pr: true

  # Loops panel - shows running agent
  - name: maestro-loops
    window_size: [1200, 800]
    agent_setup: start_demo_loop
    requires:
      - loop_running: true
```

## Key functions

```swift
// Maestro: Add --capture flag

// MaestroApp.swift - handle launch args
func handleCaptureMode() {
    // --capture <output-path> [--view <view-name>] [--size WxH]
    // 1. Wait for window to be ready
    // 2. Optionally resize
    // 3. Capture via CaptureService
    // 4. Save to specified path
    // 5. Exit
}
```

```python
# scripts/generate_screenshots.py

def setup_demo_repo(manifest: Manifest) -> Path:
    """Clone or locate demo repo, run setup commands."""

def capture_screenshot(shot: Screenshot, repo_path: Path, output_dir: Path):
    """Launch Maestro with --capture, wait for output."""

def main():
    manifest = load_manifest("scripts/screenshots.yaml")
    repo = setup_demo_repo(manifest)
    for shot in manifest.screenshots:
        capture_screenshot(shot, repo, Path("docs"))
```

## Demo repo

Use `loopflow-demos` repo: https://github.com/loopflowstudio/loopflow-demos

Script clones it if not present, then sets up screenshot-worthy state:
```bash
# Clone if needed
git clone https://github.com/loopflowstudio/loopflow-demos ~/src/loopflow-demos

# Create worktrees with realistic state
cd ~/src/loopflow-demos
wt create add-auth
wt create fix-cache

# Make commits on each branch to get ahead-of-main badges
cd ../loopflow-demos.add-auth
echo "// auth code" >> src/auth.py && git add -A && git commit -m "Add auth module"
```

The demo repo should have:
- `.claude/commands/` with standard prompts (design, implement, review, polish)
- `.lf/config.yaml` with sensible defaults
- Some source files to make the repo look real

## Agent state

For loops panel screenshots, use mock state. Add a `--mock-loops` flag to Maestro that injects sample loop data without requiring lfd:

```swift
// AppState extension for screenshot mode
func configureMockLoops() {
    loops = [
        Loop(goalName: "test-coverage", status: .running, iteration: 3),
        Loop(goalName: "docs-sync", status: .idle, iteration: 12),
    ]
    lfdConnected = true  // Show as connected
}
```

This keeps screenshot generation fast and deterministic—no real agents needed.

## Workflow

```bash
# One command to regenerate all screenshots
python scripts/generate_screenshots.py

# Or via lfops
lfops screenshots
```

Steps:
1. Clone/setup demo repo if needed
2. Create worktrees with realistic state
3. For each screenshot in manifest:
   - Launch `Maestro.app --capture docs/<name>.png --repo <demo-repo>`
   - Maestro opens, loads repo, captures, exits
4. Images land in `docs/`

## Constraints

- **Real data only.** No mock objects or fake UI state. If it can't be created via normal git/lf commands, don't screenshot it.
- **Reproducible.** Running twice produces identical screenshots (same repo state, same window size).
- **Fast.** Reuse existing demo repo if state is valid. Only recreate when needed.

## UI changes

Add Maestro launch arguments:

```
--capture <output-path>    Capture screenshot and exit
--repo <path>              Open this repo immediately (skip welcome)
--size <WxH>               Set window size before capture
--select <branch>          Select this worktree before capture
```

## Done when

```bash
python scripts/generate_screenshots.py

ls docs/maestro-*.png
# maestro-main.png
# maestro-prompt.png

# Open an image - shows real worktrees with real data
open docs/maestro-main.png
```

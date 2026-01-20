# Maestro Screenshot Pipeline

Automated screenshot generation for documentation. Point Maestro at a real demo repo, capture the actual UI, propagate to docs.

## Implementation

The pipeline consists of:

1. **Python script** (`scripts/generate_screenshots.py`) — orchestrates demo repo setup and capture
2. **YAML manifest** (`scripts/screenshots.yaml`) — defines which screenshots to take
3. **Swift screenshot mode** — `--capture` flag in Maestro with `ScreenshotWindow.swift`

## Usage

```bash
python scripts/generate_screenshots.py
```

Generates `docs/maestro-main.png` and `docs/maestro-loops.png`.

## Manifest format

```yaml
# scripts/screenshots.yaml
repo: https://github.com/loopflowstudio/loopflow-demos
local: ~/src/loopflow-demos

screenshots:
  - name: maestro-main
    window_size: [1200, 800]

  - name: maestro-loops
    window_size: [1200, 800]
    mock_loops: true
```

Fields:
- `name` — output filename (without extension)
- `window_size` — `[width, height]` in pixels
- `select_branch` — worktree branch to select before capture
- `mock_loops` — inject fake loop data for loops panel screenshots

## Maestro launch arguments

```
--capture <output-path>    Capture screenshot and exit
--repo <path>              Open this repo immediately
--size <WxH>               Set window size before capture
--select <branch>          Select this worktree before capture
--mock-loops               Inject mock loop data
```

## How it works

1. Script loads manifest from `scripts/screenshots.yaml`
2. Clones demo repo if not present, creates worktrees with commits
3. For each screenshot:
   - Launches Maestro with `--capture` and appropriate flags
   - `ScreenshotWindow.swift` loads the repo, configures state, waits for UI
   - Captures via `CaptureService` (uses `screencapture` for reliability)
   - Exits

## Demo repo setup

The script automatically:
- Clones `loopflow-demos` if `~/src/loopflow-demos` doesn't exist
- Creates worktrees `add-auth` and `fix-cache` with sample commits
- These worktrees show "ahead of main" badges in the UI

## Mock loops

`--mock-loops` injects sample loop data without requiring a running daemon:

```swift
func configureMockLoops() {
    loops = [
        Loop(goalName: "test-coverage", status: .running, iteration: 3),
        Loop(goalName: "docs-sync", status: .idle, iteration: 12),
    ]
    lfdConnected = true
}
```

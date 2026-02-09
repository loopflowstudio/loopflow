# Loops: Accumulated Runs, Stacking, and Collapse

> "each subsequent wave run after you submit an actual PR (call 'next'), the next wave run gets stacked on top. at any point there should be a way to collapse."

## What to build

When waves loop, they accumulate completed runs — each with its own branch and PR. Concerto needs to show this history, let users collapse stacked PRs into one, and absorb unpublished work into an existing PR.

Three concerns, in priority order:
1. **Run history** — see accumulated completed runs and their PRs
2. **Collapse** — combine stacked PRs into one, or absorb current work down
3. **Stimulus editing** — deferred (separate design session)

## Current state

**Works:**
- `next` stacks (branches from HEAD, not main) — `ops/next.rs:118`
- `land` merges via `gh pr merge --squash --auto` — `ops/land.rs`
- `list_wave_runs` returns run history from DB — `store/mod.rs:72`
- `LocalWaveService.listWaveRuns(waveId:)` exists, unused by Concerto
- `LocalWaveService.collapsePRs(_:)` exists, calls `POST /waves/{id}/collapse`
- `CollapsePRsResult` model exists in LoopflowCore
- `WaitingStateCard` has collapse button + confirmation dialog

**Missing:**
- No `/v0/waves/:wave_id/collapse` route registered in lfd HTTP
- No `loopflow::ops::collapse` function
- RepoState doesn't fetch or track wave runs
- No run history view in detail panel
- No absorb mechanic

## 1. Run History UI

### Detail panel segmented picker

Add a `Picker` to `WaveDetailPanel` header. Two tabs: Current (existing `blendedView`) and Runs (new history list).

```swift
// WaveDetailPanel.swift

enum DetailTab: String, CaseIterable {
    case current = "Current"
    case runs = "Runs"
}

@State private var selectedTab: DetailTab = .current
@State private var waveRuns: [WaveRun] = []
```

In the header, right-aligned:

```swift
Picker("", selection: $selectedTab) {
    Text("Current").tag(DetailTab.current)
    Text("Runs (\(waveRuns.count))").tag(DetailTab.runs)
}
.pickerStyle(.segmented)
.frame(maxWidth: 200)
```

Body switches on `selectedTab`:

```swift
if selectedTab == .current {
    blendedView  // existing code, unchanged
} else {
    WaveRunsTab(wave: wave, runs: waveRuns, onCollapse: collapsePRs)
}
```

Fetch runs when wave changes or tab switches to Runs:

```swift
.onChange(of: wave.id) { _, _ in
    if selectedTab == .runs { Task { await fetchRuns() } }
}
.onChange(of: selectedTab) { _, tab in
    if tab == .runs { Task { await fetchRuns() } }
}

private func fetchRuns() async {
    waveRuns = (try? await LocalWaveService().listWaveRuns(waveId: wave.id)) ?? []
}
```

### Run list view

New file: `Concerto/Views/WaveRunsTab.swift`

```swift
struct WaveRunsTab: View {
    let wave: WaveViewModel
    let runs: [WaveRun]
    let onCollapse: () -> Void

    private var openPRs: [WaveRun] {
        runs.filter { $0.pr?.state == .open || $0.pr?.state == .draft }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.lg) {
            // Collapse action bar (when 2+ open PRs)
            if openPRs.count >= 2 {
                collapseBar
            }

            // Absorb bar (when current branch has unpublished work + existing PR)
            if wave.commits.count > 0, wave.prURL == nil, let lastPR = openPRs.first {
                absorbBar(targetPR: lastPR)
            }

            // Run list
            ForEach(runs) { run in
                WaveRunRow(run: run)
            }
        }
    }
}
```

Each run row:

```swift
struct WaveRunRow: View {
    let run: WaveRun
    @State private var isExpanded = false

    // Compact: #3 ship ●PR #42 (open) 2m ago 3m12s
    // Expanded: + commits, diff stat, error, PR link
    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(spacing: Spacing.sm) {
                Text("#\(run.iteration)")
                    .font(.caption).fontWeight(.medium).monospacedDigit()
                Text(run.flow)
                    .font(.caption).foregroundStyle(.secondary)

                if let pr = run.pr {
                    PRBadge(pr: pr)
                }

                Spacer()

                if let duration = run.duration {
                    Text(duration)
                        .font(.caption2).foregroundStyle(.tertiary).monospacedDigit()
                }

                Text(run.relativeTime)
                    .font(.caption2).foregroundStyle(.tertiary)
            }
            .contentShape(Rectangle())
            .onTapGesture { isExpanded.toggle() }

            if isExpanded {
                // Error, branch, timestamps
            }
        }
    }
}
```

Add computed helpers to WaveRun:

```swift
// WaveRun extension (in WaveRun.swift or a new file)
extension WaveRun {
    var duration: String? {
        guard let start = startedAt, let end = endedAt else { return nil }
        let interval = end.timeIntervalSince(start)
        let minutes = Int(interval) / 60
        let seconds = Int(interval) % 60
        return "\(minutes)m\(String(format: "%02d", seconds))s"
    }

    var relativeTime: String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: endedAt ?? startedAt ?? createdAt, relativeTo: Date())
    }
}
```

### Iteration timeline strip

New file: `Concerto/Views/IterationTimeline.swift`

Always visible in the detail panel header (both tabs). A horizontal row of connected dots:

```swift
struct IterationTimeline: View {
    let runs: [WaveRun]
    let currentIteration: Int

    var body: some View {
        HStack(spacing: 2) {
            ForEach(runs.reversed()) { run in
                Circle()
                    .fill(dotColor(for: run))
                    .frame(width: 8, height: 8)

                if run.iteration != currentIteration {
                    Rectangle()
                        .fill(Color.secondary.opacity(0.3))
                        .frame(width: 8, height: 1)
                }
            }

            // Current iteration (hollow)
            Circle()
                .strokeBorder(Color.secondary, lineWidth: 1.5)
                .frame(width: 10, height: 10)
        }
    }

    private func dotColor(for run: WaveRun) -> Color {
        if let pr = run.pr {
            switch pr.state {
            case .merged: return .statusSuccess
            case .open, .draft: return .blue
            case .closed: return .statusError
            case .none: return .gray
            }
        }
        switch run.status {
        case .completed: return .gray
        case .failed: return .statusError
        default: return .gray
        }
    }
}
```

Show in header when iteration > 0 and runs exist:

```swift
// In WaveDetailPanel header, after the iteration badge:
if !waveRuns.isEmpty {
    IterationTimeline(runs: waveRuns, currentIteration: wave.iteration)
}
```

## 2. Collapse

### Mode 1: Combine N PRs into 1

Available when: wave has 2+ open PRs across its runs.

**Backend: `ops/collapse.rs` (new file)**

```rust
use std::path::Path;
use std::process::Command;

use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::engine::git::{get_default_branch, current_branch};

#[derive(Debug, Clone, Default)]
pub struct CollapseOptions {
    pub wave_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CollapseResult {
    pub new_pr_url: Option<String>,
    pub closed_prs: Vec<u64>,
}

pub fn collapse_prs(
    repo: &Path,
    options: &CollapseOptions,
    progress: &impl Progress,
) -> OpsResult<CollapseResult>
```

Algorithm:

```
1. List open PRs for this wave:
   gh pr list --author @me --state open --json number,headRefName,url
   Filter to PRs whose branch matches the wave name pattern.

2. Order by PR number (ascending = oldest first).

3. Collect all commits from all PR branches:
   For each PR branch:
     git log origin/main..origin/{branch} --format=%H
   This gives ordered commit SHAs across all stacked branches.

4. Create a fresh branch from origin/main:
   git checkout -b {wave_name}-collapsed origin/main

5. Cherry-pick all commits in order:
   git cherry-pick {sha1} {sha2} ... {shaN}
   On conflict: abort and return error (user must resolve manually).

6. Push and create PR:
   git push -u origin {branch}
   gh pr create --title "..." --body "..." --base main

7. Close old PRs:
   For each old PR: gh pr close {number} --delete-branch

8. Return CollapseResult with new PR URL and closed PR numbers.
```

**HTTP route: `POST /v0/waves/:wave_id/collapse`**

Add to `lfd/http/mod.rs`:

```rust
.route("/waves/:wave_id/collapse", post(waves::collapse_wave_handler))
```

Handler in `lfd/http/routes/waves.rs`:

```rust
pub async fn collapse_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<Json<CollapseResponse>> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let wave = /* get wave, get latest run for worktree path */;

    let result = tokio::task::spawn_blocking(move || {
        crate::ops::collapse_prs(
            &work_dir,
            &CollapseOptions {
                wave_name: Some(wave.name.clone()),
            },
            &crate::ops::NullProgress,
        )
    }).await??;

    Ok(Json(CollapseResponse {
        ok: true,
        result: CollapseResponseResult {
            new_pr_url: result.new_pr_url,
            closed_prs: result.closed_prs,
        },
    }))
}
```

Response matches what `LocalWaveService.collapsePRs` already parses:

```rust
#[derive(Serialize)]
struct CollapseResponse {
    ok: bool,
    result: CollapseResponseResult,
}

#[derive(Serialize)]
struct CollapseResponseResult {
    new_pr_url: Option<String>,
    closed_prs: Vec<u64>,
}
```

**Swift UI: Collapse bar in WaveRunsTab**

```swift
private var collapseBar: some View {
    HStack {
        VStack(alignment: .leading, spacing: 2) {
            Text("\(openPRs.count) open PRs")
                .font(.subheadline).fontWeight(.medium)
            Text("Combine into a single PR")
                .font(.caption).foregroundStyle(.secondary)
        }
        Spacer()
        Button {
            showCollapseConfirmation = true
        } label: {
            Label("Collapse", systemImage: "arrow.triangle.merge")
        }
        .buttonStyle(.borderedProminent)
        .tint(.loopflowBurgundy)
    }
    .padding(Spacing.lg)
    .background(palette.surface)
    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
}
```

Reuse the existing `WaitingStateCard` confirmation dialog pattern.

### Mode 2: Absorb down

Available when: current branch has commits (wave.commits not empty) AND no PR on current branch AND at least 1 open PR exists from a previous run.

> "take the current, potentially unpublished wave run into just everything"

This folds the current in-progress work into the most recent open PR's branch.

**Backend: `ops/absorb.rs` (new file, or add to collapse.rs)**

```rust
pub struct AbsorbOptions {
    pub target_pr_number: u64,
}

pub struct AbsorbResult {
    pub target_branch: String,
    pub commits_absorbed: usize,
}

pub fn absorb_into_pr(
    repo: &Path,
    options: &AbsorbOptions,
    progress: &impl Progress,
) -> OpsResult<AbsorbResult>
```

Algorithm:

```
1. Get the target PR's branch:
   gh pr view {number} --json headRefName -q .headRefName

2. Get current branch's new commits (not in target):
   git log origin/{target_branch}..HEAD --format=%H

3. Cherry-pick those commits onto the target branch:
   git checkout {target_branch}
   git cherry-pick {commits...}
   git push

4. Reset the current branch (delete it or point it at target):
   git checkout {target_branch}
   (wave's worktree now tracks the target branch)

5. Return AbsorbResult.
```

**HTTP route: `POST /v0/waves/:wave_id/absorb`**

New endpoint. Swift client needs a new method:

```swift
// LocalWaveService.swift
public func absorbIntoPR(_ id: String, prNumber: Int) async throws {
    let url = apiBaseURL.appendingPathComponent("waves/\(id)/absorb")
    var request = URLRequest(url: url)
    request.httpMethod = "POST"
    request.setValue("application/json", forHTTPHeaderField: "Content-Type")
    request.httpBody = try JSONSerialization.data(withJSONObject: ["pr_number": prNumber])
    let (data, response) = try await longSession.data(for: request)
    // handle errors
}
```

**Swift UI: Absorb bar in WaveRunsTab**

```swift
private func absorbBar(targetPR: WaveRun) -> some View {
    HStack {
        VStack(alignment: .leading, spacing: 2) {
            Text("\(wave.commits.count) unpublished commits")
                .font(.subheadline).fontWeight(.medium)
            Text("Add to PR #\(targetPR.pr?.number ?? 0)")
                .font(.caption).foregroundStyle(.secondary)
        }
        Spacer()
        Button {
            absorbDown(into: targetPR)
        } label: {
            Label("Absorb", systemImage: "arrow.down.to.line")
        }
        .buttonStyle(.bordered)
    }
    .padding(Spacing.lg)
    .background(palette.surface)
    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
}
```

## Key functions

| Layer | Function | File | Purpose |
|-------|----------|------|---------|
| Rust ops | `collapse_prs(repo, options, progress)` | `ops/collapse.rs` (new) | Combine N PR branches into 1 |
| Rust ops | `absorb_into_pr(repo, options, progress)` | `ops/collapse.rs` (new) | Fold current work into existing PR |
| Rust HTTP | `collapse_wave_handler` | `lfd/http/routes/waves.rs` | Expose collapse via API |
| Rust HTTP | `absorb_wave_handler` | `lfd/http/routes/waves.rs` | Expose absorb via API |
| Swift service | `absorbIntoPR(_:prNumber:)` | `LocalWaveService.swift` | Call absorb endpoint |
| Swift view | `WaveRunsTab` | `Views/WaveRunsTab.swift` (new) | Run history + collapse/absorb actions |
| Swift view | `WaveRunRow` | `Views/WaveRunsTab.swift` (new) | Single run row |
| Swift view | `IterationTimeline` | `Views/IterationTimeline.swift` (new) | Dot timeline in header |

## Files to modify

**Rust (new):**
- `rust/loopflow/src/ops/collapse.rs` — collapse + absorb ops
- Register in `rust/loopflow/src/ops/mod.rs`

**Rust (modify):**
- `rust/loopflow/src/lfd/http/mod.rs` — add collapse + absorb routes
- `rust/loopflow/src/lfd/http/routes/waves.rs` — add handlers

**Swift (new):**
- `swift/Concerto/Views/WaveRunsTab.swift` — runs tab + run rows
- `swift/Concerto/Views/IterationTimeline.swift` — dot timeline

**Swift (modify):**
- `swift/Concerto/Views/WaveDetailPanel.swift` — add segmented picker, runs state, fetch logic
- `swift/LoopflowCore/Services/LocalWaveService.swift` — add `absorbIntoPR` method

**Swift (remove/simplify):**
- `swift/Concerto/Views/WaitingStateCard.swift` — collapse logic moves to WaveRunsTab. WaitingStateCard keeps the "Review PRs" button but collapse moves to Runs tab.

## Constraints

- `next` behavior unchanged — still stacks from current HEAD
- Collapse must handle PRs that were manually merged outside Concerto (skip them, only operate on open PRs)
- Cherry-pick conflicts in collapse → abort and return a clear error, don't leave the repo in a broken state
- Run list fetched on-demand when Runs tab selected (not on every poll cycle)
- Existing tests must pass: `cargo test --all && swift test --package-path swift`

## Done when

```bash
# Backend
curl -X POST localhost:4242/v0/waves/{id}/collapse
# Returns: {"ok": true, "result": {"new_pr_url": "...", "closed_prs": [1,2,3]}}

curl -X POST localhost:4242/v0/waves/{id}/absorb -d '{"pr_number": 42}'
# Returns: {"ok": true, "result": {"target_branch": "...", "commits_absorbed": 3}}

# Tests
cargo test --all
swift test --package-path swift
```

1. Detail panel has `[Current] [Runs (N)]` segmented picker
2. Runs tab shows wave runs with PR status badges, iteration, duration
3. Iteration timeline strip in header (colored dots)
4. "Collapse" button combines 2+ open PRs into 1 (end-to-end)
5. "Absorb" button folds unpublished work into most recent open PR
6. All tests pass

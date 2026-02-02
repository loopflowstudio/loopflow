# Actionable Waiting States

Make blocked waves show the reason and next action in one place.

## Problem

When a wave hits PR limit, the UI shows "Waiting (PR limit reached)" but no path forward. Users must leave Concerto to find blocking PRs or decide what to do. Every persona sees the block without the action.

## Approach

Display waiting state as a single contextual card that shows:
1. **Why** — the specific blocking condition with counts
2. **What's blocking** — direct links to the blocking PRs
3. **Next action** — a button that resolves the most common unblock path

The card replaces the generic progress message in `WaveDetailPanel.runProgressSection`.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Separate "Blockers" panel | More UI complexity, additional navigation | Users want to act inline, not drill into another view |
| Notification toast | Ephemeral, easy to miss | Blocking conditions persist — need persistent display |
| Sidebar badge only | Surfaces the count but not the action | Matches "Needs Attention" but doesn't help resolve |
| Fetch all open PRs from GitHub | Richer context | API complexity, rate limits, latency — overkill for v1 |

## Key decisions

**Show count, not list.** Display "2/5 PRs open" rather than listing each PR. The sidebar's "Open PRs" section already groups waves with PRs — users know where to look. Clicking "Review PRs" goes to GitHub's PR list for the repo.

**"Review PRs" as primary action.** The most common unblock path is landing a PR. Since we can't know which PR should land, link to the repo's PR list. One click from blocked to GitHub PRs.

**Model change: `waitingReason`.** Add `waitingReason: WaitingReason?` to Wave model. Reason comes from daemon API. For v1, only `prLimitReached(open: Int, limit: Int)` — extensible for future reasons (e.g., `behindMain`, `ciBlocked`).

**Daemon already knows.** `start_wave()` returns `StartResult(False, "waiting", outstanding=outstanding)`. Surface this through the API: add `waiting_reason` and `open_prs` fields to the wave JSON response.

**Per VISUAL_DESIGN.md:** Use yellow accent for waiting states. Action button uses burgundy (`loopflowBurgundy`) as the CTA color. Count displays use `.monospacedDigit()` for alignment.

## Scope

In scope:
- `WaitingReason` enum in Wave model
- Updated API response with `waiting_reason`, `open_prs` fields
- WaveDetailPanel waiting state card with reason, count, and action
- "Review PRs" button opens GitHub PR list for the repo
- VoiceOver: full text read for counts and action

Out of scope:
- Listing individual blocking PRs (requires GitHub API)
- Other waiting reasons (future work)
- Sidebar reorganization (waiting waves already in "Active" section)
- Quick actions to land PRs directly from Concerto

## Implementation

### 1. Model (Swift)

```swift
// Wave.swift
public enum WaitingReason: Sendable, Hashable {
    case prLimitReached(open: Int, limit: Int)

    public var description: String {
        switch self {
        case .prLimitReached(let open, let limit):
            return "\(open)/\(limit) PRs open"
        }
    }
}

public struct Wave {
    // ... existing fields
    public var waitingReason: WaitingReason?
}
```

### 2. API (Python)

```python
# daemon/http_server.py - wave JSON response
def wave_to_json(wave: Wave) -> dict:
    data = { ... }  # existing fields
    if wave.status == WaveStatus.WAITING:
        outstanding = count_outstanding(wave)
        data["waiting_reason"] = "pr_limit_reached"
        data["open_prs"] = outstanding
    return data
```

### 3. Service (Swift)

```swift
// WaveService.swift - parseWaveFromJSON
if let reason = json["waiting_reason"] as? String, reason == "pr_limit_reached",
   let open = json["open_prs"] as? Int {
    wave.waitingReason = .prLimitReached(open: open, limit: wave.prLimit)
}
```

### 4. UI (Swift)

```swift
// WaveDetailPanel.swift - replace case .waiting in runProgressSection
case .waiting:
    WaitingStateCard(wave: wave)
```

```swift
// New: WaitingStateCard.swift
struct WaitingStateCard: View {
    let wave: Wave
    private let terminalLauncher = TerminalLauncher()

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Status line
            HStack(spacing: 8) {
                Image(systemName: "pause.circle.fill")
                    .foregroundStyle(.yellow)
                Text("Waiting")
                    .font(.headline)
            }

            // Reason with count
            if let reason = wave.waitingReason {
                Text(reason.description)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .monospacedDigit()
            }

            // Action button
            Button {
                openPRList()
            } label: {
                Label("Review PRs", systemImage: "arrow.up.right.square")
            }
            .buttonStyle(.borderedProminent)
            .tint(.loopflowBurgundy)
        }
        .padding(16)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    private func openPRList() {
        // GitHub PR list for this repo
        let repoPath = wave.repo
        // Extract owner/repo from git remote or use repo name
        if let url = URL(string: "https://github.com/\(repoOwnerRepo(from: repoPath))/pulls") {
            terminalLauncher.openURL(url)
        }
    }
}
```

## Done when

1. Wave in waiting state shows "2/5 PRs open" (not generic "PR limit reached")
2. "Review PRs" button visible and opens GitHub PR list
3. VoiceOver reads "Waiting. 2 of 5 PRs open. Review PRs button."
4. Tests verify WaitingReason parsing and display

## Files to change

| File | Change |
|------|--------|
| `swift/LoopflowCore/Models/Wave.swift` | Add `WaitingReason` enum, `waitingReason` field |
| `swift/LoopflowCore/Services/WaveService.swift` | Parse `waiting_reason`, `open_prs` from JSON |
| `swift/Concerto/Views/WaveDetailPanel.swift` | Extract waiting card to component |
| `swift/Concerto/Views/WaitingStateCard.swift` | New file - waiting state UI |
| `src/loopflow/lfd/daemon/http_server.py` | Add `waiting_reason`, `open_prs` to response |
| `swift/ConcertoTests/WaveTests.swift` | Test WaitingReason parsing |

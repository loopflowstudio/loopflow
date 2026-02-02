# Make Waiting States Actionable

Show why a wave is blocked and how to unblock it—in one place, with one click.

## Problem

When a wave enters waiting status, users see "PR limit" but can't act on it. The blocker is named; the resolution isn't. Users must leave Concerto to find blocking PRs, check their state, and decide what to do.

The data exists in lfd. It's just not surfaced.

## Approach

Replace the generic "PR limit" label with a contextual action row that shows:
1. What's blocking (count of outstanding commits vs. limit)
2. A direct action to resolve it

**Waiting row transformation:**

| Before | After |
|--------|-------|
| `• PR limit` | `2/3 PRs open` + "Review PRs" button |

The button opens GitHub filtered to the wave's open PRs—one click to the resolving action.

## Alternatives Considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Inline PR list in sidebar | Shows blocking PRs directly | Too noisy for the compact sidebar; detail panel is the right place |
| Toast notification when blocked | Draws attention | Doesn't help resolve; users still need to find PRs |
| Auto-open GitHub when blocked | Reduces clicks | Too aggressive; users may not want browser interruption |
| Show PR numbers in row | More specific | Numbers without links aren't actionable |

## Key Decisions

**Count-based framing over PR list.** "2/3 PRs open" communicates the bottleneck clearly. A list of PR numbers would clutter the row and requires more cognitive load to parse.

**Single "Review PRs" action.** One button that opens all relevant PRs in GitHub. Multiple buttons per PR would be overwhelming and rarely useful—users typically review PRs sequentially anyway.

**GitHub search URL.** Link to `github.com/{owner}/{repo}/pulls?q=author:@me+is:open` filtered by the wave's repo. This surfaces all open PRs for the user, which is what actually blocks the wave (the daemon counts commits ahead of main, which correlates to open PRs by this author).

**Italic serif for the count.** Per VISUAL_DESIGN.md, ephemeral context uses Cormorant Garamond italic. The "2/3 PRs open" is contextual status, same as activity timestamps.

**Button in secondary info line.** Keeps the row structure consistent with other states. The button is small (caption-sized) and uses a subtle style to avoid visual weight.

## Scope

**In scope:**
- Add `outstandingCount` to Wave model (lfd already computes this)
- Update WaveRow to show count + action for waiting state
- GitHub URL construction using repo info already in Wave

**Out of scope:**
- PR list in detail panel (separate backlog item)
- CI status on blocking PRs (future enhancement)
- Global PR limit across all waves (different blocking mechanism)

## Implementation

### 1. Extend lfd Wave Response

In `http_server.py`, add `outstanding_count` to wave serialization:

```python
def _wave_to_dict(wave: Wave) -> dict:
    # ... existing fields ...
    "outstanding_count": count_outstanding(wave) if wave.status == WaveStatus.WAITING else None,
```

### 2. Update Swift Wave Model

In `Wave.swift`, add:

```swift
public let outstandingCount: Int?
```

Computed property for display:

```swift
public var waitingDisplay: String? {
    guard status == .waiting, let count = outstandingCount else { return nil }
    return "\(count)/\(prLimit) PRs open"
}
```

### 3. Update WaveRow

Replace the static "PR limit" text:

```swift
if wave.status == .waiting {
    Text("•")
        .font(.caption)
        .foregroundStyle(.white.opacity(0.3))

    if let display = wave.waitingDisplay {
        Text(display)
            .font(.custom("Cormorant Garamond", size: 11))
            .italic()
            .foregroundStyle(.yellow.opacity(0.7))
    }

    Button {
        openReviewPRs()
    } label: {
        Text("Review PRs")
            .font(.caption2)
            .fontWeight(.medium)
    }
    .buttonStyle(.plain)
    .foregroundStyle(.yellow)
    .accessibilityLabel("Review open pull requests")
}
```

### 4. GitHub URL Helper

```swift
private func openReviewPRs() {
    // Construct GitHub search URL for user's open PRs in this repo
    // wave.repo gives us "owner/repo" format
    guard let repoPath = wave.repo else { return }
    let searchURL = "https://github.com/\(repoPath)/pulls?q=is:open+is:pr+author:@me"
    if let url = URL(string: searchURL) {
        NSWorkspace.shared.open(url)
    }
}
```

## Done When

A user with a waiting wave can:
1. See "2/3 PRs open" (or similar) instead of generic "PR limit"
2. Click "Review PRs" and land directly on GitHub filtered to their open PRs
3. Complete the round trip from "I see a block" to "I'm acting on it" in one click

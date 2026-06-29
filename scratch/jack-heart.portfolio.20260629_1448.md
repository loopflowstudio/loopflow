# Portfolio tiers in Concerto

## What to build

Turn Concerto's flat, lastOpened-sorted portfolio grid into **four ordered
tier sections** — Core, Active, Future, Deprecated — with manual ordering of
projects inside each tier. Tiers are hardcoded and stable; a project's "rank"
is *(tier, position-in-tier)*.

> "Tiers combine ranking and groups. The groups are ordered but the items
> within a group are not necessarily — they can still be manually sortable."

> "It's basically priorities for projects rather than tasks. This is what
> Asana had with Sections before the schema-backed priority field. Hardcode
> the 4 tiers."

Real portfolio after this lands:

| Tier | Projects |
|------|----------|
| Core | Cadenza, Loopflow |
| Active | Hootro |
| Future | Silencio, Manabot |
| Deprecated | Studio |

## Data structures

Normalized: a project **references** a tier by id and carries its own
**priority**. Tier metadata lives once in a tier table. Nothing depends on
array position — every entity is self-describing.

```swift
// Tier as DATA (seeded hardcoded today). Becoming user-editable later means
// moving `all` from a constant into stored config + CRUD — the repo model
// below does not change.
struct PortfolioTier: Codable, Hashable, Identifiable {
    let id: String          // "core", "active", "future", "deprecated"
    var displayName: String
    var order: Int          // tier rank — coarse ordering

    static let all: [PortfolioTier] = [
        .init(id: "core",       displayName: "Core",       order: 0),
        .init(id: "active",     displayName: "Active",     order: 1),
        .init(id: "future",     displayName: "Future",     order: 2),
        .init(id: "deprecated", displayName: "Deprecated", order: 3),
    ]
    static let `default` = all[1]  // Active
    static func find(_ id: String) -> PortfolioTier { all.first { $0.id == id } ?? .default }
}

struct PortfolioRepo: Codable, Identifiable, Hashable {
    let path: String
    var lastOpened: Date
    var tierId: String    // references PortfolioTier.id
    var priority: Double  // fine ordering WITHIN tier; lower = higher in list

    // Custom decode: legacy entries ({path, lastOpened}) default tierId -> "active".
    // Persisted UI state, not a wire DTO — a UX-choice default is allowed.
}
```

**Rank = `(tier.order, priority)`.** Tier gives coarse rank, priority gives
fine rank — exactly "tiers combine ranking and groups." `priority` is a
`Double` so a project can be slotted *between* two others (midpoint) without
renumbering the rest — the move that makes drag-reorder and a configurable
future natural rather than a rewrite.

## Key functions

```swift
// PortfolioService — rank lives in the data, not the array.

// Canonical order, consumed by EVERY surface (dashboard sections, add-repo
// typeahead, any future picker). Sort by (tier.order, priority), lastOpened
// desc as final tiebreak.
var orderedRepos: [PortfolioRepo] { get }

// Grouped view of orderedRepos, in tier order. Always returns all tiers
// (empty ones included) so they're valid drop targets.
func reposByTier() -> [(tier: PortfolioTier, repos: [PortfolioRepo])]

// CHANGED: dedupe + validate existence only; never re-sort by lastOpened
// (that would ignore priority). Legacy repos decode tierId -> "active".
private func normalizedRepos(_ entries: [PortfolioRepo]) -> [PortfolioRepo]

// CHANGED: new repo -> tierId "active", priority = (min priority in Active) - 1
// so it lands at the top of Active.
func addRepo(_ url: URL)

// NEW: the drag-and-drop primitive. Drop `movedPath` into `tier` at the slot
// between `above` and `below` (either may be nil at an edge). Sets tierId and
// assigns priority = midpoint(above.priority, below.priority); at an edge,
// min-1 / max+1. Reassigns one row, persists, no renumbering of others.
func reorder(_ movedPath: String, into tier: PortfolioTier,
             above: PortfolioRepo?, below: PortfolioRepo?)
```

`midpoint`: `(a + b) / 2`. Empty tier → priority `0`. Top edge → `below - 1`.
Bottom edge → `above + 1`. Doubles give ~50 splits between any two ints before
precision matters at this scale — a periodic normalize-on-load (reassign
0,1,2… per tier) is a cheap safety net, optional for v1.

## UI

`PortfolioWindow` changes from one `LazyVGrid` over `repos` to a vertical
stack of **tier sections**:

```
Core
  [Cadenza] [Loopflow]
Active
  [Hootro]
Future
  [Silencio] [Manabot]
Deprecated
  [Studio]
```

- Each section: a header (`tier.displayName`, burgundy per VISUAL_DESIGN) +
  a `LazyVGrid` of `PortfolioRepoCard`s for that tier.
- **All four tiers always render**, including empty ones — they're valid drop
  targets.
- No special treatment for Deprecated — ordinary section, same visual weight.

**Drag-and-drop is MVP.** A card is `.draggable` (payload: repo `path`). Drop
targets resolve a `(tier, above, below)` slot and call
`reorder(...)`. Two drop granularities:
- **Onto a card / between cards** — insert at that slot in that card's tier.
- **Onto a section** (incl. empty) — append to that tier.

SwiftUI: `.draggable(repo.path)` on the card; `.dropDestination(for: String.self)`
on cards (compute above/below from drop position) and on section containers
(append). Keep a context-menu **Move to tier ▸** as a keyboard/non-drag
fallback and for accessibility. Existing "Remove from portfolio" stays.

Drag across tiers changes `tierId`; drag within a tier only changes `priority`.
Both go through the single `reorder` call — the model never cares which view
gesture produced it.

## Constraints

- `PortfolioRepo` is **not a wire DTO** — Swift/UserDefaults only. No Rust/Python
  mirror, no DTO no-defaults rule. The decode default (`.active`) is allowed.
- Decode must not throw on legacy stored data (`{path, lastOpened}` with no
  `tier`). Custom `init(from:)` using `decodeIfPresent(...) ?? .active`.
- Do not re-sort by `lastOpened` anywhere in the portfolio path once tiers
  exist — it would clobber the priority order. `lastOpened` is now only a final
  tiebreak inside `orderedRepos`.
- All ordering flows through `orderedRepos` / `reorder` — no view computes its
  own sort. This is what lets a typeahead share the dashboard's order for free.
- Update `PortfolioServiceTests` and `PortfolioRepoStateTests` for the new field
  and the order-preservation guarantee.

## Done when

- Launch Concerto; existing portfolio repos appear under **Active** (legacy
  migration), nothing lost.
- **Drag** Cadenza and Loopflow into **Core**, **drag** Studio into
  **Deprecated**, reorder Cadenza above Loopflow within Core; quit and relaunch
  — tier placement and within-tier order persist.
- Sections render in fixed order Core → Active → Future → Deprecated with
  burgundy headers; empty tiers still show as drop targets.
- `swift test --package-path swift --filter Portfolio` passes (covers legacy
  decode → Active, `reorder` midpoint/edge math, `orderedRepos` sort key).

## Open (decide in implementation)

- **Pre-seed real portfolio?** Could map known repo names → tiers on first run.
  Brittle (name-matching); lean **no** — place once, it persists.
- **Hide empty tiers** vs always show all four. Lean **always show** (targets
  to move projects into).

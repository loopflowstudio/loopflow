# Loop 01 — Design Candidates

Four genuinely-distinct approaches to "find the one wave that needs me, and open
it." They differ in the load-bearing dimension — what the screen is *organized
around* and the *interaction model* — not in button placement.

- **A** organizes around **attention** (a "Needs You" band on top; repo demoted).
- **B** keeps the **shipped repo-first shape** but makes the row carry its reason.
- **C** organizes around **status as space** (a portfolio board / kanban).
- **D** removes the list from the critical path entirely (**keyboard palette**).

All four honor "frame, don't render": opening a wave lands in the terminal-first
wave screen; none of them render chat or a wave-detail panel. Where a candidate
challenges a guardrail, it's called out.

---

## Candidate A — Attention Queue First
**Bet:** The right default view is *attention*, not *repo*. If the screen leads
with "here's what needs you, and why," the behavior succeeds without the user
navigating at all. Repo becomes a real filter you reach for only to browse.

**Sketch:**
```
┌───────────────────────────── Loopflow ─────────────────────────────┐
│  ⚠ NEEDS YOU (2)                                    [ All repos ▾ ] │
│ ┌─────────────────────────────────────────────────────────────────┐│
│ │ ✕ payments-api · checkout-refactor                              ││
│ │   Failed at `gate` · iter 4 · +210 −38          [ Open ⌘↵ ]    ││
│ ├─────────────────────────────────────────────────────────────────┤│
│ │ ◐ web · onboarding-flow                                         ││
│ │   Waiting — 3/3 PRs open, land one to continue  [ Open ]       ││
│ └─────────────────────────────────────────────────────────────────┘│
│                                                                     │
│  ● RUNNING (4)                                            ⌄ collapse│
│    ● web · nav-redesign            iter 7 · +64 −12                 │
│    ● payments-api · webhook-retry  iter 2                           │
│    ● infra · tf-modules            iter 1                           │
│    ● docs · api-reference          iter 9 · +30 −4                  │
│                                                                     │
│  ○ IDLE (14)                                              ⌄ expand  │
└─────────────────────────────────────────────────────────────────────┘
```
**How the behavior plays out:** App opens straight to this. The "Needs You" band
is always at top, red/amber, count in the header. The developer reads two rows,
each stating the *reason* (`Failed at gate`, `Waiting — 3/3 PRs open`), picks the
failed one, hits `Open` (or `⌘↵` on the top row). If nothing needs them, the
band is a single green "All clear" line and their eye never leaves the top.

**Surfaces / SwiftUI:** Replaces the flat `List` with sectioned content ordered
`failed → waiting → running → idle`. Surfaces `waitingReason.description`,
`failed` step from `activeRun`, `iteration`, `diffIndicator`. Repo moves out of
the `NavigationSplitView` sidebar into a header filter menu — the sidebar's job
(pick a repo) is demoted to an optional filter, which is arguably what "repo is
a filter, not a container" actually asks for. Reuses the existing
`AttentionQueueView` / `AttentionItem` prior art in the codebase.

**Sacrifices:** Loses the always-visible repo list — Tess can't see "the
payments repo" as a standing spatial place; she filters to it. Portfolio-by-repo
mental model gets weaker in exchange for attention-by-default.

---

## Candidate B — Enriched Repo List (minimal deviation)
**Bet:** The shipped layout is *fine*; the gap is purely legibility. Keep
repo-sidebar-filters-list exactly, but make each row a rich status card and sort
the list by attention priority. Cheapest path; tests whether layout was ever the
problem.

**Sketch:**
```
┌── Loopflow ──┬──────────────────── Waves ─────────────────────────┐
│ ▣ All Repos  │  All repos · 20 waves · 2 need you    ● Connected  │
│              │ ───────────────────────────────────────────────── │
│ Repos        │ ✕  checkout-refactor            [payments-api]  ›  │
│  ▸ payments  │    Failed at gate · iter 4 · +210 −38             │
│  ▸ web       │ ─────────────────────────────────────────────────  │
│  ▸ infra     │ ◐  onboarding-flow                    [web]     ›  │
│  ▸ docs      │    Waiting — 3/3 PRs open                          │
│  ▸ mobile    │ ─────────────────────────────────────────────────  │
│              │ ●  nav-redesign                       [web]     ›  │
│              │    Running · iter 7 · +64 −12                      │
│              │ ─────────────────────────────────────────────────  │
│              │ ●  webhook-retry               [payments-api]   ›  │
│              │    Running · iter 2                                │
│              │ ○  … 14 idle …                                     │
└──────────────┴────────────────────────────────────────────────────┘
```
**How the behavior plays out:** App opens on `.all` (as it does today). The list
is sorted `failed → waiting → running → idle`, so the two attention rows sit at
top regardless of repo. Each row's second line is the *reason*, not the bare
status word. Header shows a "N need you" rollup. Developer reads the top row,
clicks it (or the `›` chevron) to open. To narrow, they pick a repo in the still-
present sidebar.

**Surfaces / SwiftUI:** Same `NavigationSplitView` and `RepoSidebarWaveRow`
shape; row gains a reason subtitle (`waitingReason` / failed step / `iteration`
+ `diffIndicator`) and a chevron affordance; `filteredWaves` gains an attention
sort comparator; header gains a `needs-you` count. The smallest possible diff
from what shipped.

**Sacrifices:** Attention is still an *emergent* property of a sorted list, not
a named place — if a third fire appears mid-scroll it's less obvious than a
counted band. Keeps the repo sidebar's real estate even for one-repo users.

---

## Candidate C — Portfolio Board (status-as-space)
**Bet:** For a portfolio, *space* beats a list. Lay waves in status columns so
"where's the fire" is answered by which column has cards in it — pre-attentive,
before reading any row. Team-lead-shaped.

**Sketch:**
```
┌───────────────── Loopflow · Portfolio ────────  [repos: all ▾] ───┐
│  NEEDS YOU (2)      RUNNING (4)        IDLE (14)                    │
│ ┌───────────────┐  ┌───────────────┐  ┌───────────────┐            │
│ │✕ checkout-ref │  │● nav-redesign │  │○ billing-cron │            │
│ │ payments-api  │  │ web · it7     │  │ payments-api  │            │
│ │ Failed @ gate │  │ +64 −12       │  └───────────────┘            │
│ └───────────────┘  └───────────────┘  ┌───────────────┐            │
│ ┌───────────────┐  ┌───────────────┐  │○ dark-mode    │            │
│ │◐ onboarding   │  │● webhook-retry│  │ web           │            │
│ │ web           │  │ payments · it2│  └───────────────┘            │
│ │ 3/3 PRs open  │  └───────────────┘  ┌───────────────┐  … +12     │
│ └───────────────┘  ┌───────────────┐  │○ …           │            │
│                    │● tf-modules   │  └───────────────┘            │
│  (empty = calm)    │ infra         │                               │
│                    └───────────────┘                               │
└────────────────────────────────────────────────────────────────────┘
```
**How the behavior plays out:** App opens to the board. The developer's eye goes
to the leftmost "NEEDS YOU" column; if it's empty, the portfolio is calm and
they're done in one second. If it has cards, each card carries its reason. Repo
is a color/chip on each card plus a top filter. Click a card → open. Cards can be
color-keyed by repo so Tess still perceives "payments is the red-chip cluster."

**Surfaces / SwiftUI:** Biggest structural departure — `LazyVGrid` / columns
instead of `List`; drops the `NavigationSplitView` sidebar as primary (repo is a
top filter). Same model fields as A. Challenges the shipped "sidebar filters a
list" shape most directly.

**Sacrifices:** Density and horizontal scanning cost more chrome; a 1-wave user
(Sol) gets a mostly-empty three-column board — heavy for a light portfolio.
Columns can't also group by repo, so you pick one spatial axis (status) and lose
the other (repo) as structure.

---

## Candidate D — Command-Palette Triage (keyboard-first, zero-UI)
**Bet:** The fastest path to the wave that needs you is no navigation at all. A
`⌘K` palette opens pre-sorted to the top attention wave; typing filters; `↵`
opens straight into the terminal wave screen. The list still exists as a
fallback, but the critical path never touches it.

**Sketch:**
```
        ┌──────────────────────────────────────────────┐
   ⌘K → │ ⌇ jump to a wave…                             │
        ├──────────────────────────────────────────────┤
        │ ✕ checkout-refactor   payments · Failed @gate │ ← preselected
        │ ◐ onboarding-flow     web · Waiting 3/3 PRs   │
        │ ● nav-redesign        web · Running it7        │
        │ ● webhook-retry       payments · Running       │
        │ ○ billing-cron        payments · Idle          │
        │ …                                              │
        │  ↑↓ move · ↵ open in terminal · ⌘↵ open list  │
        └──────────────────────────────────────────────┘
     (behind it: whatever view — list/board — is incidental)
```
**How the behavior plays out:** From anywhere, `⌘K`. The palette opens with the
highest-priority attention wave *preselected* (failed > waiting > running). The
developer reads one line, presses `↵`, and is dropped into that wave's terminal
harness pane. Zero pointer. For "is anything on fire," the palette's top row
answers it; if the top row is green, nothing is.

**Surfaces / SwiftUI:** An overlay (`.sheet` / focused overlay) over whatever
the base view is — composes *with* A, B, or C rather than replacing them. Sorts
by the same attention comparator; each row shows status + reason inline. `↵`
routes to the wave screen. Leans hard into "frame, don't render" and the
terminal-first bet.

**Sacrifices:** Invisible until you know `⌘K` exists — poor discoverability for a
newcomer who doesn't know the keystroke. Gives no ambient/glanceable portfolio
health when the app is just sitting open on a second monitor. Solves *open the
one that needs me* superbly and *is anything wrong at a glance* poorly.

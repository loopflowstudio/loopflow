---
asana_id: '1214269992184208'
---
# Governance surfaces

**Finish line:** The system-level Concerto surfaces — runboard, portfolio, calibration, beat programming, and release controls — all read from the same engine-backed model of waves, runs, attention, mutations, and schedules. No dashboard fork, no UI-only shadow state.

## Context

These surfaces live in the macOS app, but they are workflow work. They express how the engine thinks about the system:

- **Runboard** — what's happening now across waves
- **Portfolio** — what the whole system looks like at a glance
- **Calibration** — where garden output becomes a human decision
- **Beat programming** — how scheduled rhythm is composed
- **Release controls** — when and how a repo ships

If these screens invent their own data model, they drift from the actual engine. If they share the engine model, they become trustworthy.

## Portfolio: shipped, and the gap that remains

The portfolio surface now has structure. Concerto's dashboard groups repos into
four fixed tiers — Core, Active, Future, Deprecated — with manual ordering inside
each tier. Rank is `(tierId, priority)` carried on each repo; `PortfolioService.orderedRepos`
is the single canonical sort every portfolio view consumes; `reorder` assigns a
midpoint/edge `priority` so a drag touches exactly one repo, no renumbering. Drag-and-drop
plus a context-menu "Move to tier" fallback drive it. This is the pattern the *other*
surfaces should copy: one engine-owned ordered accessor, no per-view sort.

**The gap vs this item's finish line:** tier and priority live in Swift/UserDefaults,
not lfd. That is exactly the "UI-only shadow state" the finish line rejects. The
portfolio looks right but is not yet engine-backed — nothing outside the macOS app
can read or set a repo's tier. The remaining portfolio work for this item:

- Move tier membership + priority into the lfd wave/portfolio model so the portfolio
  reads the same engine state as runboard and calibration, and tier can be set from
  CLI/automation, not only by dragging in the app.
- Make `PortfolioRepo` a real wire DTO once the state moves server-side (today it's
  deliberately Swift-only, so the DTO no-defaults rule doesn't apply and a decode
  default of `.active` is allowed for legacy `{path, lastOpened}` rows).

Deferred polish (real, but not blocking the engine-backing work):

- Visible insertion-line affordance during drag — placement is currently a simple
  top-half/bottom-half split, enough for MVP but not a precise drop indicator.
- Concerto UI drag/drop test automation — model tests cover legacy decode → Active,
  `reorder` midpoint/edge math, and `orderedRepos` sort key, but the UI runner could
  not bootstrap headless, so no automated drag QA exists.
- Periodic normalize-on-load (reassign 0,1,2… per tier) if repeated midpoint inserts
  between the same neighbors ever degrade `Double` precision — cheap, unneeded at
  current portfolio scale.

Decided against: repo-name pre-seeding into tiers. Repos start in Active and are
placed once; name-matching was too brittle to be worth it.

## Daily experience

Open Concerto and the governance picture is obvious: what shipped, what is blocked, what root proposes, what cadence is running, and whether a release needs attention. Drill in anywhere and you're still reading the same underlying state.

## Done when

- Runboard, portfolio, calibration, beat programming, and release controls share one underlying model
- Garden and govern output shows up without bespoke translation logic per screen
- A reviewer can trace any UI state back to wave/run/attention/mutation data in lfd
- The surfaces help steer the system instead of merely reporting on it

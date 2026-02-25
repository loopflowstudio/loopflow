# Portfolio Dashboard Review

## What was implemented

- Replaced the launch-time `WelcomeWindow` flow with a new `PortfolioWindow` grid.
- Introduced persistent portfolio models/services (`PortfolioRepo`, `PortfolioService`) and migrated legacy `recentRepos` data into the new storage key.
- Added per-repo lightweight state (`PortfolioRepoState`) to fetch waves, compute summary stats (blocked/active/diff totals), and react to websocket events.
- Added repo discovery (`RepoScanner`) and typeahead-driven repo add flow via `~/src` main worktree scan.
- Added card interactions to:
  - open a repo window,
  - open a repo window with a selected wave,
  - remove a repo from the portfolio.
- Updated repo-window routing to accept wave selection messages from portfolio clicks.
- Added tests for portfolio persistence, repo scanning, and portfolio repo-state behavior.
- Follow-up polish in this gate pass:
  - Wave events are now routed to the matching repo card instead of broadcast to every card.
  - Repo cards now render full wave lists in a bounded scroll region (so long lists are actually scrollable).
  - Added a regression test ensuring delete events from another repo cannot remove local waves.

## Key choices

- **Reuse persistence instead of inventing another store:** `RecentsService` was replaced by `PortfolioService`, with migration from the old `recentRepos` key.
- **Keep portfolio state lightweight:** each card uses `PortfolioRepoState` rather than full `RepoState`, minimizing coupling and startup cost.
- **Cross-window wave selection uses notifications:** portfolio click opens the repo window and posts a targeted wave selection notification keyed by repo path.
- **Fail-safe event handling:** when a wave event arrives without embedded wave data, cards refresh from HTTP to recover canonical state.

## How it fits together

`ConcertoApp` now launches to `PortfolioWindow`. `PortfolioWindow` reads persisted repos from `PortfolioService`, creates a `PortfolioRepoState` per repo, and subscribes to lfd events through `EventService`. Repo cards render wave summaries from `PortfolioRepoState`; clicking a wave opens `RepoWindow` and forwards wave selection, where `RepoWindow` applies selection once repo state is loaded.

## Risks and bottlenecks

- Wave-selection delivery still relies on notification timing (immediate + delayed retry) when opening a new repo window.
- Portfolio cards snapshot the active connection at state creation time; runtime connection changes do not rebuild existing card states yet.
- If lfd emits many wave events with missing payloads, fallback full refresh per repo could become noisy.

## What's not included

- No cross-repo wave operations.
- No wave creation from the portfolio view.
- No drag-and-drop repo reordering.
- No dynamic multi-server/per-repo connection model.

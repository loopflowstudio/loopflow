# Portfolio Dashboard: Current State and Next Work

Concerto now launches into a portfolio grid that replaces `WelcomeWindow`. This doc tracks the current implementation and the remaining follow-up work for this feature.

## Current state

### Product behavior

- App launch opens `PortfolioWindow`.
- Portfolio cards show per-repo wave summaries (status, activity, diff totals) and a scrollable wave list.
- Clicking a repo opens that repo window; clicking a wave opens the repo window with that wave selected.
- `+` card opens typeahead repo picker backed by `~/src` scan of main worktrees.
- Repo cards support remove-from-portfolio actions.

### Data and state

- `RecentsService` was replaced by `PortfolioService`.
- Legacy `recentRepos` data migrates to the new portfolio storage key.
- Per-repo card state uses `PortfolioRepoState` (lighter than full `RepoState`).
- Live updates come from lfd events and are routed to matching repo cards.
- Missing wave payload events trigger repo refresh for recovery.

### Coverage

- Tests exist for:
  - portfolio persistence/migration,
  - repo scanning behavior,
  - portfolio repo-state event handling (including cross-repo delete regression).

## Remaining follow-up work

1. **Wave selection handshake reliability**
   - Current selection delivery relies on notification timing (immediate + delayed retry).
   - Follow-up: introduce a stronger readiness handshake between portfolio and repo windows.

2. **Connection changes while portfolio is open**
   - Existing cards capture connection at creation and are not rebuilt on runtime connection switch.
   - Follow-up: rebuild or rebind card states when active connection changes.

3. **Refresh pressure from sparse websocket payloads**
   - Missing payload fallback currently performs full repo refresh.
   - Follow-up: add throttling/coalescing to reduce refresh noise under bursty events.

## Scope intentionally excluded

- Cross-repo wave operations
- Wave creation from portfolio
- Drag-and-drop repo reordering
- Dynamic per-repo/per-server multi-connection model

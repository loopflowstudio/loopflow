# Portfolio Dashboard — Design Review

## What was implemented

Replaced the `WelcomeWindow` (a simple recent-repos list) with a `PortfolioWindow` — a grid dashboard that shows per-repo wave summaries with live status, blocked counts, and diff totals. Each repo card has a scrollable wave list. Clicking a wave opens the repo window with that wave selected. A `+` card opens a typeahead repo picker backed by a `~/src` scan of main worktrees.

## Key choices

**PortfolioRepoState instead of full RepoState.** Each card gets a lightweight `PortfolioRepoState` that holds just the wave list, connection status, and summary metrics. This avoids spinning up the full repo state machinery (output buffers, worktree tracking, agent sessions) for every card on the dashboard.

**Event routing via shared EventService.** A single WebSocket subscription in `PortfolioWindow` dispatches wave events to matching `PortfolioRepoState` instances by normalized repo path. This keeps the event fan-out simple and avoids per-card WebSocket connections.

**Notification-based wave selection handshake.** When a user clicks a wave in the portfolio, `PortfolioWindow` opens the repo window and posts a `.selectPortfolioWave` notification. `RepoWindow` buffers this as `pendingWaveSelection` and applies it once the repo finishes loading. A delayed retry (400ms) covers timing races where the repo window hasn't subscribed yet.

**RepoScanner filters linked worktrees.** The typeahead scans `~/src` for directories with a `.git` directory (main repos) and excludes `.git` files containing `/.git/worktrees/` paths (linked worktrees). This ensures the picker shows only main worktrees.

**Migration from RecentsService.** `PortfolioService` replaced `RecentsService` with the same persistence key pattern. `RecentRepo` model was replaced by `PortfolioRepo`. The `ScreenshotWindow` no longer receives the service at all since it doesn't need portfolio persistence.

## How it fits together

```
ConcertoApp
  └─ PortfolioWindow (grid of PortfolioRepoCards)
       ├─ PortfolioRepoState × N (lightweight per-repo state)
       ├─ EventService (single WebSocket, fans out events)
       └─ RepoTypeahead (scans ~/src via RepoScanner)
            └─ opens → RepoWindow (full state, receives wave selection via notification)
```

`PortfolioService` persists the repo list to UserDefaults. `PortfolioRepoState` fetches waves via `WaveService` on creation and receives live updates from the shared `EventService`.

## Risks and bottlenecks

1. **Wave selection timing.** The notification + 400ms retry is fragile. If the repo window takes longer than 400ms to load and subscribe, the selection is lost. The scratch doc notes this as follow-up work (explicit window-ready handshake).

2. **Connection captured at creation.** `PortfolioRepoState` instances capture the active connection at card creation time. If the user changes the server connection while the portfolio is open, existing cards won't rebind. Also noted as follow-up.

3. **Full refresh on missing payload.** When a wave event arrives without a payload, all repo states trigger a full refresh. Under bursty websocket conditions this could cause excess HTTP traffic. Throttling/coalescing is deferred.

4. **Dark mode.** Fixed during gate: `PortfolioWindow` was using hardcoded `Color.loopflowCream` instead of `palette.background`. Now uses the palette environment for proper dark mode support.

## What's not included

- Cross-repo wave operations
- Wave creation from portfolio
- Drag-and-drop repo reordering
- Connection change handling for existing cards
- Explicit window-ready handshake for wave selection

# Concerto Cleanup: Waves as the Only Abstraction

## Problem

Concerto has accumulated services and state for concepts that should live behind lfd's wave API. Worktrees, sessions, flows, prompts, directions, config, and context preview are all managed client-side with separate service classes. Most of these either duplicate lfd functionality or support the step launcher UI (being shelved).

The result: 18 service files, 3 state objects, and event handling that re-fetches all waves on every WebSocket message.

## Goal

Waves are the only abstraction boundary. Concerto talks to lfd via HTTP (`/v1`) and WebSocket (`/ws`). Everything below waves is lfd's problem.

After cleanup:
- 5 services (down from 18)
- 2 state objects (down from 3)
- Event-driven updates instead of poll-and-refetch

## What survives

### Services (LoopflowCore)

| Service | Role |
|---------|------|
| `LocalWaveService` | HTTP client for `/v1` (waves, wave_runs, wave actions) |
| `LocalEventService` | WebSocket client for `/ws` (real-time events) |
| `AuthService` | Authentication state |
| `LoggingService` | Diagnostics logging |
| `NotificationService` | macOS user notifications |

### State (Concerto)

| State | Role |
|-------|------|
| `RepoState` | Waves, lfd connection, cached sidebar groupings |
| `SessionState` | Live output lines for running waves (from WS events) |

## What goes away

### Services to delete

| Service | Why |
|---------|-----|
| `ConfigLoader` | Config is server-side. lfd reads `.lf/config.yaml`. |
| `WorktreeService` | Worktrees are below the wave abstraction. lfd manages them. |
| `SessionService` | Reads lfd.db directly via SQLite. Wave runs from `/v1/wave_runs` replace this. |
| `FlowService` | Waves have flows. No separate flow service needed — flow/step lists come from lfd. |
| `PromptService` | Supported the step launcher UI (being shelved). |
| `DirectionService` | Supported the step launcher UI (being shelved). |
| `ContextPreviewService` | Shells to `lf -c` for token preview. Shelved with launcher. |
| `TokenEstimator` | Shells to CLI for token count. Shelved with launcher. |
| `ResultsService` | Computes diff baselines for session results. Replaced by wave run data from lfd. |

### Protocols to delete

| Protocol | Why |
|----------|-----|
| `WaveServiceProtocol` | Premature abstraction. Only one implementation (LocalWaveService). Delete protocol, use concrete type. |
| `EventServiceProtocol` | Same. Only LocalEventService exists. |

### State to delete

| State | Why |
|-------|-----|
| `LauncherState` | Step launcher UI is being shelved. If we need a "configure and launch" flow later, it should call lfd for estimates. |

### Models that may need trimming

- `LoopflowConfig` — only used by ConfigLoader and LauncherState. If nothing reads it after cleanup, delete.
- `PromptCard` — only used by PromptService/LauncherState.
- `ContextPreview`, `ContextSection`, `ContextItem`, `ContextOptions`, `ContextKind` — only used by ContextPreviewService/LauncherState.
- `StepRunResult`, `StepRunBaseline`, `StepRunResultStatus` — only used by ResultsService/SessionState.

## Changes in detail

### 1. Rewrite LocalEventService: Unix socket → WebSocket

**Current**: `LocalEventService` connects to `~/.lf/lfd.sock` (Unix socket). This socket doesn't exist — lfd serves events via WebSocket at `/ws`.

**New**: Connect to `ws://127.0.0.1:2486/ws` using `URLSessionWebSocketTask`. Parse the JSON events that lfd already sends (see `rust/lfd/src/types/event.rs`).

lfd WebSocket events are typed:
```
WaveCreated { wave_id, name, timestamp }
WaveUpdated { wave_id, timestamp }
WaveDeleted { wave_id, timestamp }
WaveStarted { wave_id, wave_run_id, timestamp }
WaveStopped { wave_id, timestamp }
WaveWaiting { wave_id, wave_run_id, step, timestamp }
WorktreeUpdated { worktree, repo, branch, timestamp }
AgentStarted { agent_id, step, worktree, timestamp }
AgentEnded { agent_id, status, timestamp }
OutputLine { agent_id, text, timestamp }
```

On connect, lfd sends a `connected` message with a full wave snapshot. Use this for initial state instead of a separate `listWaves` call.

Update `LFDEvent` enum to match lfd's actual event types instead of the current `worktree/session/output/wave` categories.

### 2. Event-driven wave updates (item 6)

**Current**: Every `wave.*` event triggers `refreshWaves()` which fetches ALL waves.

**New**: `handleWaveEvent` fetches only the affected wave.

```swift
private func handleWaveEvent(_ event: WaveEvent) async {
    switch event.type {
    case .created, .updated, .started, .stopped, .waiting:
        // Fetch single wave and upsert
        if let wave = try? await waveService.getWave(event.waveId) {
            upsertWave(wave)
        }
    case .deleted:
        waves.removeAll { $0.id == event.waveId }
        if selectedWave?.id == event.waveId {
            selectedWave = nil
        }
    }
}

private func upsertWave(_ wave: Wave) {
    let oldStatus = previousWaveStatuses[wave.id]
    if oldStatus != wave.status {
        handleWaveStatusChange(wave: wave, from: oldStatus, to: wave.status)
    }
    previousWaveStatuses[wave.id] = wave.status

    if let index = waves.firstIndex(where: { $0.id == wave.id }) {
        waves[index] = wave
    } else {
        waves.insert(wave, at: 0)
    }
    if selectedWave?.id == wave.id {
        selectedWave = wave
    }
}
```

Requires adding `GET /v1/waves/{id}` support to `LocalWaveService` (already exists on the server).

### 3. Kill 60-second polling timer (item 7)

**Current**: `startAutoSyncTimer()` runs `syncAndEnrich()` every 60 seconds. This fetches worktrees, detects staleness, checks CI status — all worktree-level concerns.

**New**: Delete `startAutoSyncTimer()`, `syncAndEnrich()`, `detectStaleness()`, `fetchCIStatus()`, `autoPruneCompletedWorktrees()`. With WebSocket events working correctly, polling is unnecessary.

If we want a safety-net refetch (in case WebSocket drops events), do it at 5-minute intervals and only refetch waves, not worktrees.

### 4. Cache sidebar groupings (item 8)

**Current**: `WaveSidebar` has 5 computed properties (`blockedWaves`, `prWaves`, `recentActivityWaves`, `activeWaves`, `idleWaves`) that filter and sort on every render.

**New**: Move groupings to `RepoState` as cached properties. Recompute when `waves` array changes.

```swift
// On RepoState
struct WaveGroups {
    let blocked: [Wave]
    let pr: [Wave]
    let recentActivity: [Wave]
    let active: [Wave]
    let idle: [Wave]

    var attentionCount: Int { blocked.count + pr.count }
    var allInOrder: [Wave] { blocked + pr + recentActivity + active + idle }
}

var waveGroups: WaveGroups  // Recomputed on waves didSet
```

### 5. Clean up RepoState

Remove from RepoState:
- `worktrees`, `isRefreshingWorktrees`, `refreshMessage` — worktree state
- `config` — server-side
- `prompts`, `flows`, `directions` — launcher UI
- `selectedFlow` — launcher UI
- `worktreeService`, `configLoader`, `promptService`, `flowService`, `directionService` — removed services
- `listDebounceTask`, `autoPruneInFlight` — worktree lifecycle
- `_sessionState` weak reference — simplify event wiring
- All worktree operations (`listWorktrees`, `refreshWorktrees`, `createWorktree`, `deleteWorktree`, `createPR`, `landPR`, `landBranch`, `syncMain`, `pruneWorktrees`)
- `syncAndEnrich`, `detectStaleness`, `fetchCIStatus`, `autoPruneCompletedWorktrees`
- `refreshDirections`, `refreshFlows`, `refreshFlowsAsync`, `createFlow`, `saveFlow`, `deleteFlow`
- Event wiring: drop `eventService: (any EventServiceProtocol)?` — use concrete `LocalEventService`

Keep on RepoState:
- `currentRepo`
- `waves`, `selectedWave`
- `isLoading`, `errorMessage`
- `lfdConnected`
- `waveService` (concrete `LocalWaveService`)
- `eventService` (concrete `LocalEventService`)
- `previousWaveStatuses` (for notifications)
- `waveGroups` (cached sidebar groupings)
- Wave CRUD operations
- Event subscription
- `openRepo` (simplified — just connect to lfd, fetch waves)

### 6. Simplify SessionState

SessionState currently tracks "active sessions" with baselines and diff results (via `ResultsService`). With the launcher gone and worktrees abstracted away, SessionState shrinks to:

- Live output lines for running waves (from WS `OutputLine` events)
- Interactive session tracking (for Ghostty terminal embedding)

Remove:
- `ResultsService` usage and `StepRunBaseline`/`StepRunResult` tracking
- `loadDiffPreview` — wave runs show this data via lfd

Keep:
- `activeSessions` output buffer (for live log display)
- `interactiveSession` (for Ghostty embedding)
- Output append/query methods

### 7. View impact

Views that reference removed services or state need updating:

| View | Change |
|------|--------|
| `WaveSidebar` | Use `repoState.waveGroups` instead of local computed properties |
| `WaveDetailPanel` | Remove `WorktreeService()` instantiation. Get worktree info from wave object. |
| `AreaPicker` | Remove `WorktreeService()`. Area is just a string array on the wave. |
| `NextActionsBar` | Remove `WorktreeService()`. Actions come from wave status. |
| `ContentView` | Remove `LauncherState` from environment. Remove launcher-related views. |
| `StepRunner` | May survive for interactive sessions but needs cleanup. |
| `FlowPicker` | Flows come from lfd `/v1/flows` — inline the fetch or keep a thin helper. |

### 8. FlowService: special case

FlowService has two roles:
1. Load flows/steps from lfd API (`loadFlowsAsync`) — needed for flow picker in wave config
2. Load/save/delete flow YAML files locally — not needed if flows are server-managed

For the flow picker (choosing a flow when configuring a wave), we still need to list available flows. Options:
- Keep a thin `loadFlows` method on `LocalWaveService` (or inline it)
- Add `GET /v1/flows` to the v1 API surface (currently at legacy `/flows`)

The FlowService currently hits `/flows` (not `/v1/flows`). If we migrate the flows endpoint to v1, we can add a `listFlows()` method to `LocalWaveService` and delete `FlowService` entirely.

## Implementation order

1. **Rewrite LocalEventService** (WebSocket) — unblocks event-driven updates
2. **Add `getWave` to LocalWaveService** — fetch single wave by ID
3. **Implement `handleWaveEvent`** — event-driven wave updates (item 6)
4. **Delete auto-sync timer** and worktree-related methods (item 7)
5. **Cache sidebar groupings** on RepoState (item 8)
6. **Delete LauncherState** and all launcher-related views/services
7. **Delete removed services** (ConfigLoader, WorktreeService, SessionService, FlowService, PromptService, DirectionService, ContextPreviewService, TokenEstimator, ResultsService)
8. **Delete protocols** (WaveServiceProtocol, EventServiceProtocol)
9. **Clean up RepoState** — remove worktree/config/prompt/flow/direction state and methods
10. **Clean up SessionState** — remove ResultsService and baseline tracking
11. **Update views** that referenced removed services
12. **Delete orphaned models** (LoopflowConfig, PromptCard, ContextPreview, etc.)
13. **Move flows endpoint to v1** and add `listFlows` to LocalWaveService

## Not in scope

- Rewriting the wave detail panel UX
- Adding new lfd endpoints (except maybe `/v1/flows`)
- Changing the Ghostty terminal embedding
- Changing authentication

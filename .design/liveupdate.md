# Live Update

## What to build

Enhance Maestro SwiftUI app with live-refreshing worktree list via push notifications from lfd, plus main branch protection.

**Location**:
- `Maestro/` (SwiftUI macOS app)
- `src/loopflow/lfd/` (daemon additions)

## User quotes

> "as soon as the wt remove happens we should somehow notify the maestro app, probably via lfd"

> "it's a little more robust to react to wt than lf since users could invoke wt manually"

> "Why are we using polling vs push notifications? I think its good to respond to wt generally, but doesnt wt have hooks?"

## Architecture

```
┌─────────────────┐    post-create     ┌─────────────────┐
│   worktrunk     │ ─────────────────► │  lfd notify     │
│   (wt switch)   │    pre-remove      │  CLI command    │
└─────────────────┘                    └────────┬────────┘
                                                │
                                                ▼
                                       ┌─────────────────┐
                                       │   lfd daemon    │
                                       │  Unix socket    │
                                       │  ~/.lf/lfd.sock │
                                       └────────┬────────┘
                                                │ broadcast
                                                ▼
                                       ┌─────────────────┐
                                       │    Maestro      │
                                       │  (subscribed)   │
                                       └─────────────────┘
```

## Data structures

### lfd protocol (existing)

```python
# src/loopflow/lfd/protocol.py (existing)
@dataclass
class Event:
    event: str          # e.g. "worktree.created"
    data: dict[str, Any]
```

### Worktree events (new)

```python
# Event types
Event("worktree.created", {"branch": "feature-x", "path": "/path/to/worktree"})
Event("worktree.removed", {"branch": "feature-x"})
```

## Key functions

### 1. lfd server: add `notify` method

```python
# src/loopflow/lfd/server.py

async def _handle_notify(self, params: dict) -> Response:
    """Accept external events and broadcast to subscribers."""
    event_name = params.get("event")
    event_data = params.get("data", {})

    if not event_name:
        return error("Missing 'event' parameter")

    await self._broadcast(Event(event_name, event_data))
    return success({"event": event_name})
```

Add to `_dispatch()`:
```python
elif method == "notify":
    return await self._handle_notify(params)
```

### 2. lfd CLI: add `notify` command

```python
# src/loopflow/lfd/__init__.py

@app.command()
def notify(
    event: str = typer.Argument(help="Event name (e.g. worktree.created)"),
    branch: str = typer.Option(None, "--branch", "-b", help="Branch name"),
    path: str = typer.Option(None, "--path", "-p", help="Worktree path"),
):
    """Send an event to lfd for broadcast to subscribers."""
    if not is_running():
        return  # Silently fail if daemon not running

    data = {}
    if branch:
        data["branch"] = branch
    if path:
        data["path"] = path

    client = DaemonClient()
    try:
        asyncio.run(client.call("notify", {"event": event, "data": data}))
    except Exception:
        pass  # Best effort - don't fail hooks
```

### 3. worktrunk user hooks

```toml
# ~/.config/worktrunk/config.toml

[post-create]
lfd-notify = "lfd notify worktree.created --branch '{{ branch }}' --path '{{ worktree_path }}'"

[pre-remove]
lfd-notify = "lfd notify worktree.removed --branch '{{ branch }}'"
```

### 4. Maestro: LFDEventService

```swift
// Maestro/Maestro/Services/LFDEventService.swift

import Foundation
import Network

actor LFDEventService {
    private var connection: NWConnection?
    private let socketPath = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".lf/lfd.sock")

    func subscribe(
        to patterns: [String],
        onEvent: @escaping (String, [String: Any]) -> Void
    ) async throws {
        let params = NWParameters()
        let endpoint = NWEndpoint.unix(path: socketPath.path)
        connection = NWConnection(to: endpoint, using: params)

        connection?.start(queue: .main)

        // Send subscribe request
        let request = """
        {"method":"subscribe","params":{"events":\(patterns)}}

        """
        connection?.send(content: request.data(using: .utf8), completion: .idempotent)

        // Read events
        receiveLoop(onEvent: onEvent)
    }

    private func receiveLoop(onEvent: @escaping (String, [String: Any]) -> Void) {
        connection?.receive(minimumIncompleteLength: 1, maximumLength: 65536) { [weak self] data, _, _, _ in
            if let data = data,
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let event = json["event"] as? String,
               let eventData = json["data"] as? [String: Any] {
                onEvent(event, eventData)
            }
            self?.receiveLoop(onEvent: onEvent)
        }
    }

    func disconnect() {
        connection?.cancel()
        connection = nil
    }
}
```

### 5. Maestro: integrate in AppState

```swift
// Maestro/Maestro/AppState.swift

@MainActor
@Observable
final class AppState {
    // ... existing properties ...

    private var eventService: LFDEventService?

    func startEventSubscription() {
        eventService = LFDEventService()

        Task {
            try? await eventService?.subscribe(to: ["worktree.*"]) { [weak self] event, data in
                Task { @MainActor in
                    self?.handleWorktreeEvent(event, data: data)
                }
            }
        }
    }

    private func handleWorktreeEvent(_ event: String, data: [String: Any]) {
        // Refresh worktree list on any worktree event
        Task {
            await refreshWorktrees()
        }
    }
}
```

### 6. Remove polling from WorktreeSidebar

```swift
// Maestro/Maestro/Views/WorktreeSidebar.swift

// DELETE this .task block:
// .task {
//     while !Task.isCancelled {
//         await appState.refreshWorktrees()
//         try? await Task.sleep(for: .seconds(2))
//     }
// }
```

## Main branch protection (already implemented)

When main is selected and user clicks Run:
1. Generate random name from magical/musical words
2. Create worktree via `wt switch --create`
3. Refresh worktree list (will get push notification)
4. Select new worktree
5. Launch task

## Constraints

- **Silent failures**: `lfd notify` must not fail worktrunk hooks - catch all errors
- **No daemon = no events**: If lfd isn't running, Maestro won't get updates (acceptable)
- **Idempotent refresh**: Multiple events may arrive close together; `refreshWorktrees()` should be debounced or idempotent

## Done when

```bash
# 1. Setup worktrunk hooks
cat >> ~/.config/worktrunk/config.toml << 'EOF'
[post-create]
lfd-notify = "lfd notify worktree.created --branch '{{ branch }}' --path '{{ worktree_path }}'"

[pre-remove]
lfd-notify = "lfd notify worktree.removed --branch '{{ branch }}'"
EOF

# 2. Create worktree - Maestro updates instantly
wt switch --create test-push
# → Maestro sidebar shows "test-push" immediately (no 2s delay)

# 3. Remove worktree - Maestro updates instantly
wt remove test-push
# → Maestro sidebar removes "test-push" immediately

# 4. Main branch protection still works
# In Maestro: select main → select task → click Run
# → New worktree created with random name
# → Sidebar updates instantly
# → Task runs in new worktree
```

## Files to modify

| File | Change |
|------|--------|
| `src/loopflow/lfd/server.py` | Add `_handle_notify()` method |
| `src/loopflow/lfd/__init__.py` | Add `notify` CLI command |
| `Maestro/Maestro/Services/LFDEventService.swift` | New file - Unix socket client |
| `Maestro/Maestro/AppState.swift` | Add event subscription |
| `Maestro/Maestro/Views/WorktreeSidebar.swift` | Remove polling `.task` |

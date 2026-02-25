# 03: Multi-Client

Multiple devices connect to the same lfd. See the same waves, same sessions, same output.

## What to build

Multiple Concerto clients connect to the same lfd and see consistent state. No client awareness — just a shared view. Both clients see the transcript, both can send messages. Like two browser tabs open to the same chat.

## What's already free

lfd owns all state. Concerto is a thin client. Most multi-client already works:

- Wave list: both clients poll/subscribe via the same HTTP + WebSocket APIs
- Wave status: EventService pushes updates to all connected WebSocket clients
- Output streaming: each client streams independently from `/v0/waves/{id}/output/stream`
- Chat transcript: stored server-side, replayed on connect
- Chat input: any client can send messages to the same session

No session awareness or handoff needed. Multiple clients just connect and see the same thing.

## What needs work

### Connection UX on iOS

iOS Concerto connects to a remote lfd (no local daemon). Needs:

- Connection setup screen (host, port, optional auth token)
- Saved connection profiles (like SSH configs)
- Reconnect handling (mobile goes to background, comes back)

```swift
// In LoopflowCore
public struct ConnectionProfile: Codable, Identifiable, Sendable, Hashable {
    public let id: UUID
    public var name: String           // "My Mac", "Dev Server"
    public var connection: ServerConnection
    public var lastConnectedAt: Date?
}

public final class ConnectionProfileStore {
    public func profiles() -> [ConnectionProfile]
    public func save(_ profile: ConnectionProfile)
    public func delete(_ id: UUID)
    public func mostRecent() -> ConnectionProfile?
}
```

### Reconnect resilience

Mobile clients disconnect frequently (background, network switch, lock screen). On foreground resume:

- EventService WebSocket: reconnect with last sequence number for gap-free replay
- Output streaming: reconnect to stream endpoint
- ChatState: already has reconnect logic (sequence numbers, replay) — verify it works with real network interruption

Hook into `UIApplication.didBecomeActiveNotification` on iOS:

```swift
#if os(iOS)
NotificationCenter.default.publisher(for: UIApplication.didBecomeActiveNotification)
    .sink { _ in reconnect() }
#endif
```

### Verify concurrent clients work

lfd may not have been tested with multiple simultaneous WebSocket connections or concurrent session input. Verify:

- Two clients subscribed to the same wave's events both receive updates
- Two clients streaming the same wave's output both receive lines
- Two clients viewing the same chat session both see messages
- Sending from either client works without conflict

If lfd has single-client assumptions (e.g. one WebSocket per repo), fix those.

## Constraints

- No lfd API changes unless concurrent clients are actually broken
- No client registration, device tracking, or session awareness
- Mac client behavior must not change

## Done when

- Mac and iPhone Concerto connected to same lfd simultaneously
- Both see the same wave list and status updates in real time
- Starting a wave on Mac shows output on iPhone
- Chat transcript visible on both devices, messages from either device appear on both
- iPhone reconnects gracefully after backgrounding

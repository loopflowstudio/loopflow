# 05: Concerto Remote Connection

Point Concerto at a remote lfd. Same protocol, different host.

## What exists after this

Concerto connects to an lfd running on a remote machine. Wave list, detail, create, run, stop, land, logs — all work over the network. WebSocket events update the UI in real time.

## Context

Concerto talks to lfd via `LocalWaveService` (HTTP) and `LocalEventService` (WebSocket), both hardcoded to `http://127.0.0.1:2486`. The protocol is already network-ready — JSON over HTTP, events over WebSocket. Nothing reads the local filesystem.

`WaveServiceProtocol` already abstracts wave operations. Adding a remote connection is configuration, not architecture.

**From Phase 03 (shipped):** lfd accepts `Authorization: Bearer <token>` on all non-loopback requests when `auth.provider=static`. The Python client already implements this (`token=` kwarg, `LFD_TOKEN` env). Concerto needs the same pattern — inject the token into HTTP requests and WebSocket upgrade headers.

**From Phase 04 (shipped):** Remote deployment uses Caddy on :443 for TLS termination, proxying to lfd:2486 internally. Concerto connects to `https://<host>:443`, not `http://<host>:2486`. Caddy uses `tls internal` (self-signed certs) — Concerto must handle certificate trust (TOFU: trust on first use, pin the cert fingerprint).

## Implementation

### Connection configuration

```swift
// LoopflowCore/Models/ServerConnection.swift

public struct ServerConnection: Codable, Sendable {
    public let host: String        // "127.0.0.1" or "ec2-host"
    public let port: Int           // 2486 local, 443 remote (Caddy TLS)
    public let token: String?      // bearer token (nil for local)
    public let useTLS: Bool        // false for local, true for remote

    public var httpBaseURL: URL {
        let scheme = useTLS ? "https" : "http"
        return URL(string: "\(scheme)://\(host):\(port)")!
    }

    public var wsBaseURL: URL {
        let scheme = useTLS ? "wss" : "ws"
        return URL(string: "\(scheme)://\(host):\(port)/ws")!
    }

    public var isRemote: Bool { host != "127.0.0.1" }

    public static let local = ServerConnection(
        host: "127.0.0.1", port: 2486, token: nil, useTLS: false
    )
}
```

### Auth header injection

```swift
// LocalWaveService — add token to requests

private func makeRequest(_ url: URL, method: String = "GET") -> URLRequest {
    var request = URLRequest(url: url)
    request.httpMethod = method
    if let token = connection.token {
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
    }
    return request
}
```

Same for WebSocket connection in `LocalEventService`:
```swift
var request = URLRequest(url: connection.wsBaseURL)
if let token = connection.token {
    request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
}
let task = session.webSocketTask(with: request)
```

### Parameterize services

`LocalWaveService` and `LocalEventService` currently hardcode the URL. Change constructors to accept `ServerConnection`:

```swift
public init(connection: ServerConnection = .local) {
    self.connection = connection
    self.baseURL = connection.httpBaseURL.appendingPathComponent("v0")
    // ... existing setup
}
```

### Timeouts for WAN

Local defaults (3s request, 10s resource) are too tight for WAN. Scale by connection type:

```swift
private var requestTimeout: TimeInterval {
    connection.host == "127.0.0.1" ? 3 : 10
}
private var resourceTimeout: TimeInterval {
    connection.host == "127.0.0.1" ? 10 : 30
}
```

### Reconnection

WebSocket will drop more often over WAN. The existing reconnection logic in `LocalEventService` needs:

- Exponential backoff (1s → 2s → 4s → 8s → 30s cap)
- Visual indicator in Concerto when disconnected
- Auto-reconnect on network change (NWPathMonitor)

```swift
import Network

private let monitor = NWPathMonitor()

func startMonitoring() {
    monitor.pathUpdateHandler = { [weak self] path in
        if path.status == .satisfied {
            self?.reconnectIfNeeded()
        }
    }
    monitor.start(queue: .global())
}
```

### Settings UI

Minimal for now — a connection settings view where you enter host, port, and token:

```swift
struct ConnectionSettingsView: View {
    @State private var host = "127.0.0.1"
    @State private var port = "2486"
    @State private var token = ""
    @State private var useTLS = false

    var body: some View {
        Form {
            TextField("Host", text: $host)
            TextField("Port", text: $port)
            SecureField("Token", text: $token)
            Toggle("Use TLS", isOn: $useTLS)
            Button("Connect") { connect() }
        }
    }
}
```

Store connection in UserDefaults or Keychain (token specifically in Keychain).

### Repo discovery

lfd already knows its repos. Concerto queries for available repos on connect instead of requiring manual path entry:

```swift
// GET /v0/repos → list of repos lfd manages
let repos = try await service.listRepos()
// Each repo includes its path, name, and active waves
```

Concerto shows a repo picker when connecting to a new server. No manual path entry.

### Daemon discovery (Phase 07)

With studio auth, manual host/port entry is replaced by auto-discovery. After sign-in, Concerto queries `GET /api/v1/daemons/discover` which returns a list of the user's registered daemons (machine_id, machine_name, url, last_heartbeat_at). Concerto shows a daemon picker when multiple machines are available, then connects to the selected one.

## Constraints

- **TLS required**: Remote connections use HTTPS/WSS. Self-signed certs (Phase 04 Caddy) trusted via cert pinning on first connect.
- **Single server**: Concerto connects to one lfd at a time. Multi-server is future work (daemon picker from Phase 07 discovery is the first step).
- **No offline mode**: If the connection drops, show disconnected state. Don't cache stale data.

## Done when

- Concerto connects to a remote lfd via HTTP+WS
- Wave list loads from remote
- Creating, running, stopping, landing waves works
- Log streaming works (may have higher latency)
- WebSocket events update UI in real time
- Connection settings UI exists (host, port, token)
- Disconnection shows clear error state with retry
- Local mode still works as default

# Bundled Container

## What to build

Replace the bundled native `lfd` binary with a bundled Docker container. Concerto launches a container instead of forking a process. Same ephemeral port/token pattern, but with filesystem isolation: lfd sees `~/src/` read-only, agents see one repo each read-write.

## Trust hierarchy

```
Concerto          full host access, Keychain, UI
   │
   ├── credential socket (Unix)
   ├── ~/src/ bind mount (read-only)
   ├── Docker socket
   │
   ▼
lfd container     sees ~/src/ (ro), Docker socket, credential socket
   │
   ├── ~/src/repo bind mount (read-write, one repo)
   ├── credentials injected as env vars
   │
   ▼
agent container   sees one repo (rw), injected credentials
```

lfd is trusted — it's our software. It reads repo state (waves, configs, `.lf/`) but doesn't write to repos directly. All file mutations happen through agent containers. Agents are semi-trusted — LLM-driven, scoped to their repo. An agent can't see other repos, can't reach the Docker socket, can't touch the host filesystem outside its mount.

## Data structures

### Rust: credential socket client

New file: `rust/loopflow/src/lfd/credential_socket.rs`

```rust
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialResponse {
    pub token: String,
    pub login: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CredentialSocketClient {
    socket_path: PathBuf,
}

impl CredentialSocketClient {
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    /// Fetch credential for a provider from the Concerto-hosted Unix socket.
    pub async fn get_credential(&self, provider: &str) -> Result<CredentialResponse> {
        // HTTP GET over Unix socket to /credentials/{provider}
        // Use hyper with unix socket connector or tokio UnixStream
    }

    /// Check if the socket is reachable.
    pub async fn health(&self) -> bool {
        // GET /health on the socket
    }
}
```

### Rust: socket-backed auth broker

New struct in `rust/loopflow/src/lfd/provider_auth.rs`. Implements the existing `AuthBroker` trait:

```rust
#[async_trait]
pub trait AuthBroker: Send + Sync {
    fn provider(&self) -> Provider;
    async fn start_auth(&self) -> Result<AuthFlowHandle, AuthError>;
    async fn check_status(&self) -> Result<AuthStatus, AuthError>;
    async fn disconnect(&self) -> Result<(), AuthError>;
}
```

```rust
#[derive(Debug, Clone)]
pub struct SocketAuthBroker {
    provider_name: Provider,
    client: Arc<CredentialSocketClient>,
}

impl SocketAuthBroker {
    pub fn new(provider: Provider, client: Arc<CredentialSocketClient>) -> Self {
        Self { provider_name: provider, client }
    }
}

#[async_trait]
impl AuthBroker for SocketAuthBroker {
    fn provider(&self) -> Provider { self.provider_name }

    async fn start_auth(&self) -> Result<AuthFlowHandle, AuthError> {
        // POST /auth/{provider}/start on credential socket
        // Concerto handles the OAuth flow; returns AuthFlowHandle with verification_uri
    }

    async fn check_status(&self) -> Result<AuthStatus, AuthError> {
        // GET /credentials/{provider} on socket
        // If token present → AuthStatus::Active { login }
        // If no token → AuthStatus::None
    }

    async fn disconnect(&self) -> Result<(), AuthError> {
        // DELETE /credentials/{provider} on socket
        // Concerto removes from Keychain
    }
}
```

Register in `ProviderAuthService::new()` when `LFD_CREDENTIAL_SOCKET` is set. Replace the default CLI-based brokers:

```rust
impl ProviderAuthService {
    pub fn new() -> Self {
        if let Ok(socket_path) = std::env::var("LFD_CREDENTIAL_SOCKET") {
            let client = Arc::new(CredentialSocketClient::new(PathBuf::from(socket_path)));
            return Self::with_brokers(vec![
                Arc::new(SocketAuthBroker::new(Provider::GitHub, client.clone())),
                Arc::new(SocketAuthBroker::new(Provider::Claude, client.clone())),
                Arc::new(SocketAuthBroker::new(Provider::Codex, client.clone())),
            ]);
        }
        // Fall back to CLI-based brokers
        Self::with_brokers(vec![
            Arc::new(GhAuthBroker::default()),
            Arc::new(ClaudeAuthBroker::default()),
            Arc::new(CodexAuthBroker::default()),
        ])
    }
}
```

### Rust: config changes

In `rust/loopflow/src/lfd/config.rs`, add `LFD_CREDENTIAL_SOCKET` to `apply_env_overrides()`:

```rust
// In apply_env_overrides()
if let Ok(value) = std::env::var("LFD_CREDENTIAL_SOCKET") {
    let trimmed = value.trim();
    if !trimmed.is_empty() {
        self.credential_socket = Some(trimmed.to_string());
    }
}
```

Add field to `RawLfdConfig`:

```rust
#[derive(Debug, Clone, Default, Deserialize)]
struct RawLfdConfig {
    // ... existing fields ...
    #[serde(default)]
    credential_socket: Option<String>,
}
```

### Rust: credential injection into agent containers

In `rust/loopflow/src/lfd/executor/docker/io.rs`, extend `collect_env()`. When credential socket is configured, fetch tokens and inject as env vars:

```rust
pub(super) fn collect_env(&self) -> Vec<String> {
    let mut env = /* existing logic */;

    // If credential socket is available, fetch and inject provider tokens
    if let Some(ref socket_client) = self.credential_socket_client {
        // These are blocking calls cached at session start, not per-collection
        for (provider, env_var) in [
            ("github", "GH_TOKEN"),
            ("claude", "ANTHROPIC_API_KEY"),
            ("codex", "OPENAI_API_KEY"),
        ] {
            if let Ok(cred) = self.cached_credentials.get(provider) {
                env.push(format!("{env_var}={}", cred.token));
            }
        }
    }

    env
}
```

### Swift: credential socket server

New file: `swift/Concerto/Platform/macOS/Services/CredentialSocketServer.swift`

Small HTTP server bound to a Unix socket. Serves credentials from Keychain.

```swift
import Foundation
import Security

final class CredentialSocketServer {
    private let socketPath: URL
    private var listener: Task<Void, Never>?

    static let defaultPath = FileManager.default.temporaryDirectory
        .appendingPathComponent("concerto-auth-\(ProcessInfo.processInfo.processIdentifier).sock")

    init(socketPath: URL = CredentialSocketServer.defaultPath) {
        self.socketPath = socketPath
    }

    func start() throws {
        // Bind Unix domain socket at socketPath
        // Listen for HTTP requests
        // Route: GET /credentials/{provider} → readKeychain(provider)
        // Route: GET /health → 200 OK
        // Route: POST /auth/{provider}/start → trigger auth flow, return verification_uri
        // Route: DELETE /credentials/{provider} → removeKeychain(provider)
    }

    func stop() {
        listener?.cancel()
        try? FileManager.default.removeItem(at: socketPath)
    }

    private func readKeychain(provider: String) -> CredentialResponse? {
        // Map provider to Keychain service name:
        //   "github" → service: "gh:github.com"
        //   "claude" → service: "Claude Safe Storage", account: "Claude"
        //   "codex"  → service: "Codex Safe Storage", account: "Codex"
        //
        // Use SecItemCopyMatching pattern from ConnectionSecretStore:
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: keychainService(for: provider),
            kSecAttrAccount as String: keychainAccount(for: provider),
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess, let data = result as? Data,
              let token = String(data: data, encoding: .utf8) else {
            return nil
        }
        return CredentialResponse(token: token)
    }

    private func keychainService(for provider: String) -> String {
        switch provider {
        case "github": return "gh:github.com"
        case "claude": return "Claude Safe Storage"
        case "codex": return "Codex Safe Storage"
        default: return ""
        }
    }

    private func keychainAccount(for provider: String) -> String {
        switch provider {
        case "github": return ""  // gh uses service-only lookup
        case "claude": return "Claude"
        case "codex": return "Codex"
        default: return ""
        }
    }
}

struct CredentialResponse: Codable {
    let token: String
    let login: String?
    let expires_at: String?
}
```

### Swift: BundledDaemonManager container mode

Modify `swift/Concerto/Platform/macOS/Services/BundledDaemonManager.swift`. Replace `Process` lifecycle with Docker CLI invocation. Same state machine (`.stopped` → `.starting` → `.running` → `.failed`), same health check, different transport.

```swift
// New properties
private var containerName: String?
private var credentialServer: CredentialSocketServer?

func start() async throws -> ServerConnection {
    // ... existing early returns for .running / .starting ...

    state = .starting

    do {
        let token = try generateSessionToken()
        let port = try allocatePort()
        let containerName = "concerto-lfd-\(token.prefix(8))"

        // 1. Start credential socket server
        let credentialServer = CredentialSocketServer()
        try credentialServer.start()

        // 2. Check Docker availability
        guard try await dockerAvailable() else {
            throw ManagerError.dockerNotAvailable
        }

        // 3. Launch container via docker CLI
        let args = buildDockerRunArgs(
            containerName: containerName,
            port: port,
            token: token,
            credentialSocketPath: credentialServer.socketPath,
            srcPath: srcDirectory()
        )
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/local/bin/docker")
        process.arguments = args
        try process.run()
        process.waitUntilExit()

        guard process.terminationStatus == 0 else {
            throw ManagerError.containerStartFailed(process.terminationStatus)
        }

        self.containerName = containerName
        self.credentialServer = credentialServer
        self.token = token
        self.port = port

        try await waitForHealth()
        let connection = try requireRuntimeConnection()
        state = .running
        return connection
    } catch {
        stop()
        state = .failed(error)
        throw error
    }
}

func stop() {
    state = .stopped

    // Stop container
    if let name = containerName {
        let stop = Process()
        stop.executableURL = URL(fileURLWithPath: "/usr/local/bin/docker")
        stop.arguments = ["stop", "-t", "3", name]
        try? stop.run()
        stop.waitUntilExit()

        let rm = Process()
        rm.executableURL = URL(fileURLWithPath: "/usr/local/bin/docker")
        rm.arguments = ["rm", "-f", name]
        try? rm.run()
        rm.waitUntilExit()
    }

    credentialServer?.stop()
    resetRuntime()
}

private func buildDockerRunArgs(
    containerName: String,
    port: Int,
    token: String,
    credentialSocketPath: URL,
    srcPath: URL
) -> [String] {
    var args = [
        "run", "-d",
        "--name", containerName,
        "-p", "127.0.0.1:\(port):2486",
        "-v", "\(srcPath.path):/workspace/src:ro",
        "-v", "/var/run/docker.sock:/var/run/docker.sock",
        "-v", "\(credentialSocketPath.path):/var/run/concerto-auth.sock",
        "-e", "LFD_HTTP_ADDR=0.0.0.0:2486",
        "-e", "LFD_DB_PATH=/data/concerto.db",
        "-e", "LFD_AUTH_PROVIDER=static",
        "-e", "LFD_AUTH_TOKEN=\(token)",
        "-e", "LFD_CREDENTIAL_SOCKET=/var/run/concerto-auth.sock",
        "-e", "LFD_MODE=container",
    ]

    // SSH and git config mounts
    let home = FileManager.default.homeDirectoryForCurrentUser
    let sshDir = home.appendingPathComponent(".ssh")
    if FileManager.default.fileExists(atPath: sshDir.path) {
        args += ["-v", "\(sshDir.path):/root/.ssh:ro"]
    }
    let gitconfig = home.appendingPathComponent(".gitconfig")
    if FileManager.default.fileExists(atPath: gitconfig.path) {
        args += ["-v", "\(gitconfig.path):/root/.gitconfig:ro"]
    }

    // Extra mounts from config
    if let config = loadConcertoConfig(), let mounts = config.container?.mounts {
        for mount in mounts {
            let expanded = (mount.path as NSString).expandingTildeInPath
            let mode = mount.readOnly ? "ro" : "rw"
            args += ["-v", "\(expanded):/workspace/extra/\(URL(fileURLWithPath: expanded).lastPathComponent):\(mode)"]
        }
    }

    args += ["loopflow/lfd:latest", "serve"]
    return args
}

private func dockerAvailable() async throws -> Bool {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/local/bin/docker")
    process.arguments = ["info", "--format", "{{.ServerVersion}}"]
    process.standardOutput = FileHandle.nullDevice
    process.standardError = FileHandle.nullDevice
    try process.run()
    process.waitUntilExit()
    return process.terminationStatus == 0
}

private func srcDirectory() -> URL {
    // Default to ~/src; configurable via concerto.yaml
    let home = FileManager.default.homeDirectoryForCurrentUser
    return home.appendingPathComponent("src")
}
```

New error cases to add to `ManagerError`:

```swift
case dockerNotAvailable
case containerStartFailed(Int32)

// errorDescription:
case .dockerNotAvailable:
    return "Docker is not running. Install Docker Desktop or start the Docker daemon."
case .containerStartFailed(let code):
    return "Failed to start lfd container (exit code \(code))."
```

### Swift: config extension for extra mounts

Extend `ConcertoConfig` in `swift/LoopflowCore/Config/ConcertoConfig.swift`:

```swift
struct ConcertoConfig: Codable {
    var connection: RemoteConnectionConfig?
    var container: ContainerConfig?
}

struct ContainerConfig: Codable {
    var mounts: [ExtraMount]?
    var image: String?  // Override image name, defaults to "loopflow/lfd:latest"
}

struct ExtraMount: Codable {
    var path: String     // e.g. "~/Documents/specs"
    var readOnly: Bool   // parsed from ":ro" / ":rw" suffix, default true
}
```

Config file format (`~/.lf/concerto.yaml`):

```yaml
container:
  mounts:
    - ~/Documents/specs:ro
    - ~/data:rw
```

Parse in the existing `parseConcertoConfig()` function alongside the `connection:` section.

### Swift: Docker availability UI

When `dockerAvailable()` returns false in `BundledDaemonManager.start()`, Concerto should:

1. Show a clear error in the connection settings: "Docker is not running"
2. Offer a "Use Native Mode" fallback button that launches the old `Process`-based daemon
3. Link to Docker Desktop install page

This uses the existing `DaemonState.failed(Error)` path — the `ManagerError.dockerNotAvailable` case just needs a specific UI treatment in `ConnectionSettingsView`.

## Key functions

| Function | File | What it does |
|----------|------|--------------|
| `CredentialSocketClient::get_credential()` | `credential_socket.rs` | HTTP GET over Unix socket, returns token |
| `SocketAuthBroker::check_status()` | `provider_auth.rs` | Queries socket, maps to AuthStatus |
| `ProviderAuthService::new()` | `provider_auth.rs` | Detects `LFD_CREDENTIAL_SOCKET`, picks broker type |
| `collect_env()` | `docker/io.rs` | Injects cached socket credentials as env vars |
| `CredentialSocketServer.start()` | `CredentialSocketServer.swift` | Binds Unix socket, serves Keychain tokens |
| `CredentialSocketServer.readKeychain()` | `CredentialSocketServer.swift` | SecItemCopyMatching for provider tokens |
| `BundledDaemonManager.start()` | `BundledDaemonManager.swift` | Starts credential server, launches Docker container |
| `BundledDaemonManager.stop()` | `BundledDaemonManager.swift` | docker stop + rm, stop credential server |
| `buildDockerRunArgs()` | `BundledDaemonManager.swift` | Assembles mount/env/port args for docker run |
| `dockerAvailable()` | `BundledDaemonManager.swift` | Checks `docker info` exits 0 |

## Mounts

### lfd container

| Host path | Container path | Mode | Purpose |
|-----------|---------------|------|---------|
| `~/src/` | `/workspace/src` | **ro** | All repos — lfd reads waves, configs, `.lf/` |
| Docker socket | `/var/run/docker.sock` | rw | lfd spawns agent containers |
| Credential socket | `/var/run/concerto-auth.sock` | ro | Auth proxy from Concerto |
| Named volume | `/data` | rw | SQLite database, persistent across restarts |
| `~/.ssh/` | `/root/.ssh` | ro | Git operations (push/pull) |
| `~/.gitconfig` | `/root/.gitconfig` | ro | Git identity |
| Extra mounts (config) | `/workspace/extra/{name}` | per-config | User-configured directories |

### Agent containers (spawned by lfd)

| Source (lfd sees) | Agent container path | Mode |
|-------------------|---------------------|------|
| `/workspace/src/repo` | `/workspace` | rw |
| (credentials from socket → env vars) | `GH_TOKEN`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY` | — |
| `/root/.ssh` | `/home/agent/.ssh` | ro |
| `/root/.gitconfig` | `/home/agent/.gitconfig` | ro |
| `/workspace/extra/*` | `/workspace/extra/*` | same as lfd |

## Extra mounts

Users can mount additional host directories into the container. Default off. Configured in `~/.lf/concerto.yaml`:

```yaml
container:
  mounts:
    - ~/Documents/specs:ro
    - ~/data:rw
```

Mounts are global — they appear in lfd and all agent containers at the same path under `/workspace/extra/`. No per-agent customization. This is for things like shared reference docs, datasets, or config directories that aren't under `~/src/`.

Concerto surfaces this in connection settings as an "Additional Directories" list with add/remove controls.

## Isolation guarantees

**What's isolated:**
- Agents can only see the repo they're working on, not other repos or host files
- lfd can see `~/src/` but read-only — it can't modify repos, only agents can
- lfd can't see `~/Documents`, `~/Downloads`, etc.
- Process isolation — runaway agent can't kill host processes
- Network namespace — agents can't scan local network
- Resource limits (memory/CPU) prevent a spinning agent from freezing the laptop

**What's explicitly shared:**
- `~/src/` into lfd (read-only, all repos)
- One repo per agent (read-write)
- Docker socket into lfd (not into agents)
- SSH keys and git config (read-only)
- Extra mounts if configured (global, default off)

**What's NOT shared:**
- macOS Keychain (accessed only by Concerto via socket proxy)
- Host filesystem outside `~/src/` (unless extra mounts configured)
- Host processes
- Host network (beyond published ports)

## Image distribution

Pull on first launch: `docker pull loopflow/lfd:latest`. Simple, always current. Adds ~30s to first launch only.

## Docker Desktop dependency

Concerto gains a dependency on Docker Desktop (or Colima/OrbStack). When Docker is not available:

- Show clear message: "Docker is not running"
- Offer native mode fallback (today's behavior, no isolation)
- Link to Docker Desktop install page

## Constraints

- Docker Desktop must be installed and running
- `~/src/` must be in Docker Desktop's file sharing settings (it is by default on macOS)
- First launch is slower (~5s container start vs ~100ms process fork)
- No macOS Keychain access from within lfd — all auth goes through the socket

## Done when

- `BundledDaemonManager` launches lfd as a Docker container with `~/src/` mounted read-only
- Credential socket serves OAuth tokens from Keychain to containerized lfd
- lfd auth broker uses socket when `LFD_CREDENTIAL_SOCKET` is set, falls back to CLI brokers otherwise
- Agent containers spawned by lfd see only their assigned repo (read-write) and injected credentials
- Extra mounts from `~/.lf/concerto.yaml` are passed through to lfd and agent containers
- Fallback to native mode when Docker is not available
- `cargo test --all` and `swift test --package-path swift` pass

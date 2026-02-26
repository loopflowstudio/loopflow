# Concerto file-based connection config

Install Concerto to `/Applications` and have it connect to a remote lfd via `~/.lf/concerto.yaml`, with token in Keychain. Default remains bundled lfd.

## What to build

Concerto reads `~/.lf/concerto.yaml` at startup. If a `connection` key is present, it seeds the connection as remote mode instead of bundled. Token lives in macOS Keychain (existing `ConnectionSecretStore` infra). Studio writes both the YAML and the Keychain entry.

## Config format

`~/.lf/concerto.yaml`:

```yaml
connection:
  host: lfd-dev.loopflow.studio
  port: 443
```

No `tls` field — remote is always TLS. No `token` field — token lives in Keychain.

If the file is missing or has no `connection` key, behavior is unchanged (bundled lfd).

## Data structures

```swift
// New: parsed from ~/.lf/concerto.yaml
struct ConcertoConfig: Codable {
    var connection: RemoteConnectionConfig?
}

struct RemoteConnectionConfig: Codable {
    var host: String
    var port: Int
}

extension RemoteConnectionConfig {
    func toServerConnection() -> ServerConnection {
        ServerConnection(
            host: host,
            port: port,
            useTLS: true,
            authMode: .staticToken
        )
    }
}
```

## Key functions

```swift
// Load ~/.lf/concerto.yaml, return nil if missing or empty
func loadConcertoConfig() -> ConcertoConfig?

// In ConnectionStore.loadInitialState():
// 1. Check UserDefaults (existing behavior, wins if present)
// 2. Check ~/.lf/concerto.yaml (new, seeds first launch)
// 3. Fall back to bundled (existing default)
```

## Startup priority

1. **UserDefaults** — if the user has previously configured a connection (via UI or prior launch), respect it. Existing behavior unchanged.
2. **`~/.lf/concerto.yaml`** — if no UserDefaults and YAML has `connection`, start in remote mode with that connection. Look up token from Keychain.
3. **Bundled** — no UserDefaults, no YAML. Default.

The YAML seeds the first launch. Once Concerto persists to UserDefaults, the YAML is not re-read (unless UserDefaults are cleared). This means studio can update the YAML and the user can `defaults delete` to pick up changes.

## Keychain integration

Token stored via existing `ConnectionSecretStore`:
- Service: `studio.loopflow.connection.token`
- Account: `<host>:<port>` (e.g. `lfd-dev.loopflow.studio:443`)

Studio writes the token from Python:

```python
subprocess.run([
    "security", "add-generic-password",
    "-s", "studio.loopflow.connection.token",
    "-a", "lfd-dev.loopflow.studio:443",
    "-w", token,
    "-U",  # update if exists
])
```

Concerto reads it via `ConnectionSecretStore.token(for:)` — no changes needed to the secret store.

## Changes by repo

### loopflow.remote (this repo)

**Swift — `LoopflowCore/`:**
- New `Config/ConcertoConfig.swift` — `ConcertoConfig`, `RemoteConnectionConfig`, `loadConcertoConfig()`
- Edit `State/ConnectionStore.swift` — in `loadInitialState()`, after checking UserDefaults and before the bundled fallback, check `loadConcertoConfig()`. If it has a connection, return `remoteState(from:)`.

That's it. ~50 lines of new Swift, ~10 lines of changed Swift.

**No changes to:**
- `install.py` — already installs Concerto to `/Applications`
- `ConnectionSecretStore` — already has the right Keychain service/account scheme
- `ServerConnection` — already has all needed fields
- Any Rust code

### studio (separate repo)

After `install.py local` completes, studio runs a setup script that:
1. Writes `~/.lf/concerto.yaml` with lfd-dev host/port
2. Writes token to Keychain via `security add-generic-password`

## Constraints

- Remote connection is always TLS. No `tls` field in config.
- Token never appears in the YAML file. Keychain only.
- UserDefaults always wins over YAML (user's manual config takes precedence).
- `localhost` remote is not supported. Local dev uses bundled daemon.

## Done when

1. Fresh Concerto launch (no UserDefaults) with `~/.lf/concerto.yaml` present → starts in remote mode, connects to the configured host
2. Fresh Concerto launch without `~/.lf/concerto.yaml` → starts bundled (unchanged)
3. Concerto launch with existing UserDefaults and `~/.lf/concerto.yaml` → UserDefaults wins
4. `swift test --package-path swift` passes with new tests covering YAML loading and startup priority

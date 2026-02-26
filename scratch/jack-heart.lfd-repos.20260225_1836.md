# Provider Auth for lfd

## What to build

Auth brokering for lfd's three providers: GitHub, Claude, and Codex. All three follow the same pattern: lfd launches the tool's own auth command, captures the auth URL, forwards it to the user's browser, and tracks status. The tools store their own credentials on the filesystem. lfd bind-mounts credential dirs into containers.

"Connect GitHub in Concerto" → browser opens → done. Same for Claude and Codex.

## One pattern

lfd doesn't own any tokens. Each tool manages its own credentials:

| Provider | Auth command | Credential dir | Container mount |
|----------|-------------|---------------|-----------------|
| GitHub | `gh auth login` | `~/.config/gh/` | `/home/agent/.config/gh/` |
| Claude | `claude` (login flow) | `~/.claude/` | `/home/agent/.claude/` |
| Codex | `codex login --device-auth` | `~/.codex/` | `/home/agent/.codex/` |

lfd is a broker, not a credential store:

1. Launches the tool's auth command
2. Captures the auth URL from stdout
3. Forwards it to Concerto (WebSocket) or `lfq` (prints + opens browser)
4. Monitors the process — when it exits, checks credential dir
5. Tracks auth status (active / none / expired)

## User experience

### Concerto: "Connections" panel

Three provider cards. Each shows status + action button.

**Connect flow (all three identical UX):**
1. User clicks "Connect"
2. Browser opens automatically (Concerto calls `open` on the URL)
3. User completes auth in browser
4. Card updates to show connected status via WebSocket event

Zero typing. Zero copy-paste.

### lfq

```
$ lfq auth status
GitHub    ✓ @jackdanger
Claude    ✓ authenticated
Codex     ✗ not connected

$ lfq auth github
Opening GitHub in your browser...
✓ Authenticated as @jackdanger

$ lfq auth claude
Opening Claude auth in your browser...
✓ Authenticated

$ lfq auth codex
Opening Codex auth in your browser...
✓ Authenticated
```

### Status check

lfd checks auth status by probing the credential directories:

- **GitHub**: run `gh auth status` or check `~/.config/gh/hosts.yml` exists with a github.com entry
- **Claude**: check `~/.claude/` exists and non-empty (existing `has_claude_credentials()` pattern)
- **Codex**: check `~/.codex/` exists and non-empty

No DB storage needed. Status is derived from filesystem state.

### Disconnect

- **GitHub**: `gh auth logout`
- **Claude**: remove `~/.claude/` credential files
- **Codex**: remove `~/.codex/` credential files

## HTTP endpoints

All authenticated (inside auth middleware).

```
GET    /v0/auth                → list all provider statuses
GET    /v0/auth/:provider      → single provider status
POST   /v0/auth/:provider      → start auth flow
DELETE /v0/auth/:provider      → disconnect
```

### `GET /v0/auth` — all providers

Probes credential dirs on the fly. No DB query.

```json
{
  "providers": [
    { "provider": "github", "status": "active", "login": "jackdanger" },
    { "provider": "claude", "status": "active" },
    { "provider": "codex", "status": "none" }
  ]
}
```

### `POST /v0/auth/:provider` — start auth flow

Launches the tool's auth command, parses stdout for the auth URL.

Response:
```json
{
  "provider": "github",
  "verification_uri": "https://github.com/login/device",
  "verification_uri_complete": "https://github.com/login/device?user_code=ABCD-1234",
  "user_code": "ABCD-1234",
  "expires_in": 900
}
```

Fields vary by provider — `user_code` may be null for Claude if it uses a redirect flow. The key field is `verification_uri_complete` (or `verification_uri`) — that's what Concerto opens in the browser.

lfd starts a background task monitoring the auth process. When it completes:

```json
// WebSocket event
{ "type": "auth.connected", "provider": "github", "login": "jackdanger" }
```

### `DELETE /v0/auth/:provider` — disconnect

Runs the tool's logout command or removes credential files.

```json
// WebSocket event
{ "type": "auth.disconnected", "provider": "github" }
```

## Auth brokering implementation

### The broker pattern

```rust
#[async_trait]
pub trait AuthBroker: Send + Sync {
    /// Launch the tool's auth command, return the URL to open
    async fn start_auth(&self) -> Result<AuthFlowResponse>;

    /// Check if credentials exist and are valid
    async fn check_status(&self) -> Result<AuthStatus>;

    /// Remove credentials
    async fn disconnect(&self) -> Result<()>;
}
```

Three implementations: `GhAuthBroker`, `ClaudeAuthBroker`, `CodexAuthBroker`.

### GitHub (`gh auth login`)

```rust
impl AuthBroker for GhAuthBroker {
    async fn start_auth(&self) -> Result<AuthFlowResponse> {
        // gh auth login --web spawns a device flow
        // stdout contains the URL and code
        // We need to figure out the right flags to make it non-interactive
        // and capture the URL instead of opening a browser directly
        let mut cmd = Command::new("gh");
        cmd.args(["auth", "login", "--web", "--git-protocol", "https"]);
        // Set GH_BROWSER=echo to capture the URL instead of opening it
        cmd.env("GH_BROWSER", "echo");
        // Parse stdout for the URL
        // ...
    }

    async fn check_status(&self) -> Result<AuthStatus> {
        // gh auth status --hostname github.com
        let output = Command::new("gh")
            .args(["auth", "status", "--hostname", "github.com"])
            .output().await?;
        // Parse for username, token status
    }

    async fn disconnect(&self) -> Result<()> {
        Command::new("gh")
            .args(["auth", "logout", "--hostname", "github.com"])
            .output().await?;
        Ok(())
    }
}
```

Key trick: `GH_BROWSER=echo` makes `gh` print the URL instead of opening it. We capture it and forward to Concerto.

### Claude

```rust
impl AuthBroker for ClaudeAuthBroker {
    async fn start_auth(&self) -> Result<AuthFlowResponse> {
        // Launch claude in a mode that triggers login
        // Set BROWSER=echo or equivalent to capture the URL
        // Parse stdout for the OAuth URL
        // TBD: exact flags and stdout format
    }

    async fn check_status(&self) -> Result<AuthStatus> {
        // Check ~/.claude/ for credential files
        let claude_dir = home_dir().join(".claude");
        if claude_dir.exists() && !is_empty(&claude_dir) {
            AuthStatus::Active
        } else {
            AuthStatus::None
        }
    }

    async fn disconnect(&self) -> Result<()> {
        // Remove credential files from ~/.claude/
        // Be careful to only remove auth files, not config/settings
    }
}
```

### Codex

```rust
impl AuthBroker for CodexAuthBroker {
    async fn start_auth(&self) -> Result<AuthFlowResponse> {
        // codex login --device-auth
        // Outputs URL + code to stdout
        let mut cmd = Command::new("codex");
        cmd.args(["login", "--device-auth"]);
        // Parse stdout for URL and user_code
    }

    async fn check_status(&self) -> Result<AuthStatus> {
        let codex_dir = home_dir().join(".codex");
        if codex_dir.exists() && !is_empty(&codex_dir) {
            AuthStatus::Active
        } else {
            AuthStatus::None
        }
    }

    async fn disconnect(&self) -> Result<()> {
        // codex logout, or remove ~/.codex/ credential files
    }
}
```

## Rust types

```rust
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Provider {
    GitHub,
    Claude,
    Codex,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AuthStatus {
    Active { login: Option<String> },
    Pending,
    None,
    Expired,
}

#[derive(Debug, Clone)]
pub struct AuthFlowResponse {
    pub provider: Provider,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub user_code: Option<String>,
    pub expires_in: Option<u64>,
}
```

## Container credential injection

The Docker executor already supports credential bind mounts. Currently configured via `executor.credentials.mounts` in `lfd.yaml`. The `resolve_credential_mount()` function maps names to paths:

```rust
"claude" | ".claude" => (&[".claude", ".claude.json"], read_write)
"codex"  | ".codex"  => (&[".codex"], read_write)
```

**New:** add `gh` to the recognized mounts:

```rust
"gh" | ".config/gh" => (&[".config/gh"], read_only)
```

GitHub credentials are read-only in containers — agents use `gh` for git operations but shouldn't modify the auth config.

For `gh` to work inside containers, the container image needs `gh` installed. This is already likely the case if we're using `gh` for PR operations.

## WebSocket events

```json
{ "type": "auth.flow_started", "provider": "github", "verification_uri_complete": "https://..." }
{ "type": "auth.connected", "provider": "github", "login": "jackdanger" }
{ "type": "auth.connected", "provider": "claude" }
{ "type": "auth.failed", "provider": "codex", "error": "expired" }
{ "type": "auth.disconnected", "provider": "github" }
```

## What this doesn't include

- **Repos** — `POST /v0/repos` with NWO, cloning, repo-as-first-class-object. Separate design.
- **Concerto UI** — the Connections panel. Separate implementation.
- **Gemini CLI auth** — same pattern, add later.
- **Token refresh** — tools handle their own refresh. lfd just re-checks status.
- **Multi-user** — one set of credentials per lfd instance.

## Open questions

- Exact `claude` CLI flag for login-only mode. Need to test.
- Does `codex login --device-auth` work with piped stdout?
- Does `GH_BROWSER=echo gh auth login --web` reliably capture the URL?
- Should lfd auto-detect missing credentials and prompt? (e.g., wave run fails because Claude isn't authed → emit WebSocket event → Concerto shows "Connect Claude" prompt)

## Done when

```bash
# All providers disconnected
curl http://localhost:9119/v0/auth
# → all "none"

# Connect GitHub
curl -X POST http://localhost:9119/v0/auth/github
# → { "verification_uri_complete": "https://...", "user_code": "ABCD-1234" }
# (complete in browser)
curl http://localhost:9119/v0/auth/github
# → { "status": "active", "login": "jackdanger" }

# Connect Claude
curl -X POST http://localhost:9119/v0/auth/claude
# → { "verification_uri": "https://claude.ai/..." }
# (complete in browser)
curl http://localhost:9119/v0/auth/claude
# → { "status": "active" }

# Connect Codex
curl -X POST http://localhost:9119/v0/auth/codex
# → { "verification_uri": "...", "user_code": "XXXX-XXXX" }
# (complete in browser)
curl http://localhost:9119/v0/auth/codex
# → { "status": "active" }

# Disconnect
curl -X DELETE http://localhost:9119/v0/auth/github
curl http://localhost:9119/v0/auth/github
# → { "status": "none" }

# lfq
lfq auth status
lfq auth github
lfq auth claude
lfq auth codex
```

Plus:
- `gh` credential mount added to Docker executor (`resolve_credential_mount`)
- WebSocket events emitted for all state changes
- Startup: `GET /v0/auth` returns correct status based on filesystem probing

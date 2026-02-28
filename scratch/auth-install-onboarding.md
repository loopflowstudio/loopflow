# Install Onboarding

## Problem

`lfd install` finishes without connecting any providers. On a fresh machine, users get a running daemon that can't do anything — no GitHub for PRs, no Claude or Codex for agents. They have to discover `lfq auth` separately. First-run should guide through auth so the system is usable immediately.

Wave goal: "`lfd install` guides new users through provider auth in one shot."

## Approach

Split onboarding into two pieces that compose cleanly with existing infrastructure:

**1. `lfd install` calls onboarding after starting the service.**

After `lfd install` finishes its existing work (writing the plist/unit, loading the service), it runs an interactive auth onboarding loop before returning. The loop talks to the now-running lfd server via its HTTP API — same path as `lfq auth`.

The onboarding loop lives in `lfd` itself (Rust), using `reqwest` (already a dependency) to call the local lfd HTTP endpoints. This keeps install self-contained — no dependency on Python/lfq being installed yet.

**2. `lfq auth status` gets richer output.**

The status table adds token expiry countdown and refresh schedule when that data is available from the API.

### Onboarding flow

```
$ lfd install

Installed ~/Library/LaunchAgents/com.loopflow.lfd.plist
lfd service loaded (native)

Connecting accounts...

  Claude — go to https://console.anthropic.com/settings/keys and enter code: ABCD-1234
  ✓ Connected as jack@anthropic.com

  GitHub — go to https://github.com/login/device and enter code: 5678-WXYZ
  ✓ Connected as @jackdoe

  Codex (optional) — press Enter to skip, or 'y' to connect:
  Skipped

  OpenCode Zen (optional) — press Enter to skip, or 'y' to connect:
  Skipped

Ready. Run `lf` to start.
```

### Provider ordering and requirements

| Provider | Required | Why |
|----------|----------|-----|
| Claude | Yes (one of Claude/Codex) | Agent provider |
| GitHub | Yes | PRs, CI, git operations |
| Codex | No | Alternative agent provider |
| OpenCode Zen | No | Alternative agent provider |

Logic:
1. Run Claude auth first (default agent provider)
2. Run GitHub auth second (always required)
3. If Claude failed/skipped, require Codex (need at least one agent)
4. Offer remaining optional providers with skip-by-default

### HTTP client in install path

The onboarding code makes three kinds of HTTP calls against `http://127.0.0.1:2486`:

- `POST /v0/auth/{provider}` → starts auth flow, gets `verification_uri` + `user_code`
- `GET /v0/auth/{provider}` → polls status until `active` or timeout
- `GET /v0/auth` → final check that required providers are connected

Uses `reqwest::blocking::Client` since the install path is synchronous. Waits up to 5s after service start for lfd to be ready (health check poll).

### `--no-interactive` flag

`lfd install --no-interactive` skips the entire onboarding loop. Prints "Run `lfq auth status` to connect providers." and exits. For CI, Docker builds, scripted installs.

### `lfq auth status` improvements

Extend `AuthProviderStatus` with optional fields from the API:

```
┌──────────────┬──────────┬─────────────────────────────────┐
│ provider     │ status   │ details                         │
├──────────────┼──────────┼─────────────────────────────────┤
│ Claude       │ ✓ active │ jack@anthropic.com · expires 4h │
│ GitHub       │ ✓ active │ @jackdoe                        │
│ Codex        │ ✗ none   │ not connected                   │
│ OpenCode Zen │ ✓ active │ zen@user.com · refresh in 12m   │
└──────────────┴──────────┴─────────────────────────────────┘
```

Changes needed:
- Rust API: add `expires_at` and `next_refresh_at` to `AuthProviderStatusDto`
- Python model: add optional `expires_at: Optional[datetime]` to `AuthProviderStatus`
- CLI display: format expiry as relative time ("expires 4h", "expires 12m"), show refresh schedule

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Shell out to `lfq auth` from `lfd install` | Simpler — reuse Python CLI | Requires Python/lfq installed at install time. `lfd install` may run before `uv tool install loopflow`. Creates circular dependency. |
| Direct broker usage in install (no server) | Self-contained, no HTTP | Requires async runtime, store setup, event hub in the install path. Duplicates the entire auth service wiring. Tokens wouldn't persist through the same store the server uses. |
| Separate `lfd onboard` command | Clean separation | Extra step users must know about. The whole point is that install guides you through everything. Could still add later as a re-run mechanism. |
| Browser auto-open | Nicer UX on desktop | Breaks SSH sessions, headless servers. Device flow with manual URL + code works everywhere. |

## Key decisions

**Rust HTTP client, not Python subprocess.** `lfd install` must work before `lfq` is installed. The install script downloads `lfd` first; Python tooling comes later. Using `reqwest::blocking` keeps the install path self-contained with no runtime dependencies beyond lfd itself.

**Blocking HTTP, not async.** The install command path is synchronous today. Adding tokio runtime just for HTTP polling is overkill. `reqwest::blocking` works fine for sequential auth flows.

**Claude first, not GitHub.** Claude is the default agent — the thing you came here to use. Get that connected first. GitHub second. If Claude was skipped, fall through to Codex requirement.

**Skip-by-default for optional providers.** Press Enter to skip. Type 'y' to connect. Don't make users work to skip things they don't want. This is the opposite of the wave item's "skip with Y/n" — defaulting to connect forces users to actively dismiss each optional provider, which is annoying with 2+ optional providers.

**Onboarding is re-runnable.** `lfd install` (even re-install) detects already-connected providers and skips them. If Claude is already active, it moves straight to GitHub. This makes re-running install safe and useful for adding providers later.

**No `lfd onboard` command yet.** Users can use `lfq auth <provider>` to connect individual providers. A dedicated `lfd onboard` that re-runs the full guided flow could come later if needed, but it's not necessary for the initial implementation.

## Scope

**In scope:**
- Onboarding loop in `lfd install` (Rust, `reqwest::blocking`)
- `--no-interactive` flag to skip onboarding
- Health check polling after service start (wait for lfd ready)
- Skip already-connected providers on re-install
- `lfq auth status` showing token expiry and refresh schedule
- `expires_at` and `next_refresh_at` fields in auth status API response

**Out of scope:**
- Separate `lfd onboard` command (use `lfq auth` for individual providers)
- Browser auto-open (print URL + code, user opens manually)
- Plan/tier detection (just show login and expiry)
- Token encryption beyond filesystem permissions (wave vision: "not here")
- Multi-user token isolation (wave vision: "not here")

## Done when

```bash
# Fresh install walks through auth
lfd install
# → installs service, starts it, guides through Claude + GitHub auth
# → offers optional Codex / OpenCode Zen
# → exits with "Ready. Run `lf` to start."

# Re-install skips already-connected providers
lfd install
# → skips Claude (already active), skips GitHub (already active)
# → "All required providers connected. Ready."

# Non-interactive skips auth entirely
lfd install --no-interactive
# → installs service, prints "Run `lfq auth status` to connect providers."

# Status shows expiry info
lfq auth status
# → shows "expires 4h" for tokens with expiry, "refresh in 12m" for scheduled refreshes
```

Advancing wave goals:
- "Tokens survive lfd restarts" — already shipped (Phase 01), onboarding feeds into it
- "`lfd install` guides new users through provider auth in one shot" — this is the goal
- "Existing installs with filesystem-based auth continue working" — onboarding only runs on fresh install or re-install, doesn't disrupt existing setups

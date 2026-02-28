# Install Onboarding + Auth Status Expiry

## What was implemented

Two features that compose:

1. **`lfd install` onboarding loop** — After `lfd install` starts the service, an interactive auth flow guides users through connecting providers (Claude, GitHub, optionally Codex/OpenCode Zen). Uses `reqwest::blocking` against the local lfd HTTP API. `--no-interactive` skips it entirely.

2. **`lfq auth status` expiry display** — Auth status now shows token expiry and refresh schedule. Added `expires_at` and `next_refresh_at` to `ProviderAuthSnapshot` (Rust), `AuthProviderStatusDto` (API), and `AuthProviderStatus` (Python model). CLI formats as relative time ("expires 4h", "refresh in 12m").

## Key choices

| Decision | Why |
|----------|-----|
| Rust HTTP client (`reqwest::blocking`), not Python subprocess | `lfd install` runs before `lfq`/Python is installed. Self-contained. |
| Blocking HTTP, not async | Install path is synchronous. Adding tokio runtime for polling is overkill. |
| Claude first, GitHub second | Claude is the default agent — the thing you came to use. |
| Skip-by-default for optional providers | Enter to skip, 'y' to connect. Don't make users dismiss things they don't want. |
| `next_refresh_at = expires_at - 20min` | Simple heuristic. The 20-minute lead (`TOKEN_REFRESH_LEAD_SECONDS`) gives the background refresh task time to act before expiry. |
| Unix timestamps in `ProviderAuthSnapshot`, ISO strings in API | Internal representation stays numeric (matches DB `expires_at` column). HTTP layer converts via `format_unix_timestamp` → `OffsetDateTime` → RFC3339. |

## How it fits together

```
lfd install
  └─ dispatch(Install)          # writes plist/unit, starts service
  └─ onboarding::run_install_onboarding()
       └─ OnboardingClient      # reqwest::blocking → http://127.0.0.1:2486
            ├─ wait_until_ready()   # health check poll, 5s timeout
            ├─ POST /v0/auth/{provider}  # start device flow
            └─ GET  /v0/auth/{provider}  # poll until active/expired/timeout

lfq auth status
  └─ GET /v0/auth → AuthProvidersResponse
       └─ status_dto(ProviderAuthSnapshot)
            └─ expires_at, next_refresh_at serialized as ISO 8601
  └─ Python _status_details() formats relative deltas
```

The onboarding module is isolated — it only depends on `Provider` from `provider_auth` and `session_token::token_path()` for auth header resolution. No shared state with the async server.

## Risks and bottlenecks

- **Health check timeout (5s)** — If lfd is slow to start (cold container pull, DB migration), onboarding may bail before the server is ready. The error message is actionable ("Run `lfq auth status` to connect providers once the daemon is running.").
- **Auth poll timeout (5 min)** — Capped at `min(provider expires_in, 5 min)`. If a provider's device flow takes longer, the user gets a timeout message but can retry with `lfq auth <provider>`.
- **`next_refresh_at` is a static estimate** — Computed as `expires_at - 20min` at query time, not from the actual background refresh scheduler. If the refresh task uses different timing, the display may be slightly off.

## What's not included

- No `lfd onboard` command — users can re-run `lfd install` or use `lfq auth <provider>` individually.
- No browser auto-open — device flow with URL + code works over SSH.
- No plan/tier detection — just shows login and expiry.
- Wave item `wave/auth/04-install-onboarding.md` deleted — feature is shipped.

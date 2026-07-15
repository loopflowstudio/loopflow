# OAuth account routing

## What to build

Let one operator connect several self-owned Claude and Codex OAuth accounts,
run each provider session against one account, and route the next launch to a
healthy account when the current account reaches its limit.

> "Loopflow Studio is just me."

The model is provider accounts owned by the current operator. It contains no
company, employee, seat, or cross-person pooling concept.

## The demo

```sh
lf auth connect claude --account primary
lf auth connect claude --account reserve
lf auth use claude primary
lf auth accounts claude
```

Two independent browser logins appear in `lf auth accounts`. New Claude
sessions use `primary`; after a rejected rate-limit event, `primary` shows its
reset time and the next launch starts a new provider session with `reserve`.
The same flow works for Codex. `lf ssh mini -- lf wave product` carries both
accounts for that remote process tree and leaves no credential files behind.

## Data structures

```rust
struct ProviderAccount {
    provider: Provider,             // Claude or Codex
    id: ProviderAccountId,          // operator-chosen stable slug
    home: Option<PathBuf>,           // local native home; absent when forwarded
    login: Option<String>,
    enabled: bool,
    preferred: bool,
    utilization_percent: Option<u8>,
    cooldown_until: Option<i64>,
    cooldown_reason: Option<String>,
    last_selected_at: Option<i64>,
}

struct ProviderSessionAccount {
    provider: Provider,
    provider_session_id: String,
    account_id: ProviderAccountId,
}

struct ForwardedProviderCredential {
    provider: Provider,
    account_id: ProviderAccountId,
    access_token: SecretString,
}

struct ProviderAccountRoute {
    account_id: ProviderAccountId,
    credential: NativeHome(PathBuf) | AccessToken(SecretString),
    resume_requested_session: bool,
}
```

SQLite stores account metadata, health, and provider-session pins, but no new
OAuth secret material. Local vendor homes remain the credential source of
truth. The SSH bundle exists only in the environment inherited by the remote
process tree.

## Key behavior

### Login and shared configuration

`lf auth connect <claude|codex> --account <slug>` creates a mode-0700 profile
under `~/.lf/accounts/<provider>/<slug>` and drives the vendor's browser/device
OAuth flow in that home.

- Claude launches with `CLAUDE_CONFIG_DIR=<profile>`. The profile links its
  `skills`, `commands`, `plugins`, and settings to the canonical `~/.claude`
  configuration while keeping credentials, projects, cache, and sessions
  account-local.
- Codex launches with `CODEX_HOME=<profile>` and file credential storage. Its
  config/rules link to `~/.codex`; Loopflow skills remain the canonical
  `~/.agents/skills` tree.
- Existing behavior remains ambient when no managed account exists.

`lf auth accounts`, `lf auth use`, `lf auth enable`, `lf auth disable`,
`lf auth reset`, and account-scoped `lf auth disconnect` provide explicit
control. `use` sets the preferred account; routing falls back automatically.

### Selection and session identity

For a new provider session, select enabled accounts that are not cooling by:
lowest observed utilization, preferred account, oldest selection time, then
account id. Atomically update `last_selected_at`.

When resuming, the provider-session pin wins while that account is healthy. If
it is cooling, choose another account and discard the vendor resume id: a
conversation is never resumed under a different account. Persist the new pin
as soon as the vendor announces its session id. Codex uses `thread/resume` for
healthy pinned sessions; Claude uses `--resume`.

### Rate limits

Read Claude `rate_limit_event` and Codex `account/rateLimits/updated` frames.
Record utilization and reset time for the active account. A warning updates
health without interrupting work. A rejected/reached limit marks the account
cooling and emits a terminal provider-rate-limit error. Existing supervision
relaunches the work; routing then creates a fresh provider session on the next
healthy account. If none is healthy, fail with the earliest reset time instead
of repeatedly launching a known-limited account.

### SSH credential lease

> "Process-lifetime credentials: preserve current 'bring auth, leave nothing
> behind' semantics, but re-forward after remote restart."

`lf ssh` resolves every enabled local Claude/Codex profile, serializes only its
provider, account id, and current access token, and writes the bundle through
SSH stdin. It never appears in argv or logs. The remote router prefers this
bundle over host-local profiles and injects only the chosen account into the
vendor child (`CLAUDE_CODE_OAUTH_TOKEN` or `CODEX_ACCESS_TOKEN`).

Provider-child restarts reuse the inherited lease. A Wave/tmux/host restart
destroys it; the next local start or reconnect resolves and forwards a fresh
bundle. Automatic recovery after host restart requires a live local Loopflow
agent to reconnect. The remote never reconstructs credentials alone.

## Constraints

- OAuth accounts only; API-key routing is unchanged.
- Raw tokens never reach terminal output, logs, argv, SQLite account metadata,
  or the design/review artifacts.
- Compiled skills remain one canonical tree per host; profile setup links rather
  than copies.
- Account switching occurs only at a provider-session boundary.
- Rate-limit parsers tolerate sparse/unknown fields from newer vendor CLIs.

## Done when

1. Store tests prove multiple same-provider accounts, one preferred account,
   atomic deterministic selection, cooldown expiry, and session pinning.
2. CLI tests prove account slug validation and status rendering; command help
   exposes connect/list/use/enable/disable/reset/disconnect flows.
3. Profile tests prove Claude/Codex homes are mode 0700, independent auth state
   is selected through the correct environment, and shared configuration/skills
   are linked rather than copied.
4. Harness tests prove healthy sessions keep their account, a cooling pin starts
   without the old resume id, Codex sends `thread/resume`, and hard Claude/Codex
   rate-limit frames mark the active account cooling.
5. SSH tests prove all enabled OAuth accounts travel only in the stdin preamble,
   Codex and Claude are both represented, quoting cannot execute input, and no
   profile home or refresh token is forwarded.
6. With no configured accounts, existing Claude/Codex and `lf ssh` behavior is
   unchanged.
7. README examples describe multi-account OAuth and remote credential lifetime.
8. `cargo fmt --check`, targeted account/auth/harness/SSH tests, and
   `cargo clippy -p loopflow --all-targets -- -D warnings` pass.

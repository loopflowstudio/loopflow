# Auth

lfd becomes a real OAuth token broker. Tokens in the DB, proactive refresh, credential injection into executors, guided onboarding at install.

## Vision

Today lfd delegates auth entirely to provider CLIs (`gh auth login`, `claude auth login`, `codex login`). It captures the device flow URL, infers status from filesystem heuristics, and never touches the actual tokens. This works on a developer laptop where credential files already exist. It breaks in three places:

- **Containers.** No keychain, no host filesystem. Agents can't auth.
- **Fresh machines.** `lfd install` finishes without connecting any providers. First wave run fails with a cryptic auth error.
- **Long-running waves.** OAuth tokens expire mid-run. No refresh, no recovery. The agent silently 401s.

The auth wave replaces filesystem-heuristic auth with DB-backed token management. Tokens flow into lfd's database once at login. A background task refreshes them before expiry. Executors pull from the DB at launch — no host mounts, no env vars assumed from the host shell.

### Not here

- Multi-user token isolation (single operator per lfd instance)
- lfd implementing OAuth PKCE flows directly — provider CLIs remain the auth initiator
- Token encryption beyond filesystem permissions (0600 on lfd.db is sufficient for single-user)

## Goals

- Tokens survive lfd restarts
- Token refresh happens without user intervention
- Container agents get credentials without host filesystem mounts
- `lfd install` guides new users through provider auth in one shot
- Existing installs with filesystem-based auth continue working (DB is primary, filesystem is fallback)

## Phase boundaries

- **01-token-store**: DB-backed persistence. Tokens captured at auth completion, status reads DB first, filesystem fallback preserved. **Shipped.**
- **02-proactive-refresh**: Background task refreshes tokens before expiry. Event emission for success/failure. Graceful fallback when provider CLI doesn't support refresh. **Next.**
- **03-credential-injection**: Both local and Docker executors pull tokens from DB at agent launch. No host env dependency for API keys. **Shipped.**
- **04-install-onboarding**: `lfd install` includes interactive provider auth setup. At least one agent provider + GitHub required to complete.

## Risks

- **Provider refresh paths vary.** `gh auth refresh` exists. `claude auth refresh` may not. Phase 02 must detect absent refresh commands and fall back to emitting an `auth.refresh_required` event for the user to re-auth manually.
- **Token format drift.** Claude's `~/.claude/.credentials.json` and Codex's `~/.codex/auth.json` are not documented APIs. Parse defensively, log warnings on unexpected formats, don't block auth on extraction failure.
- **GitHub SSH vs HTTPS.** Token injection covers API calls, but `git push` over SSH still needs host credential config. Injection supplements mounts — doesn't replace them.
- **Concurrent refresh races.** Two refresh attempts for the same provider could race. The background task should hold a per-provider lock during refresh.

## Metrics

- `lfq auth status` shows `active` for all connected providers after lfd restart (tokens survived in DB)
- Background refresh fires at least once when a token is within threshold of expiry
- A container-mode agent run succeeds with credentials injected from DB (no host env var set)
- `lfd install` on a clean machine prompts for provider auth and completes without errors

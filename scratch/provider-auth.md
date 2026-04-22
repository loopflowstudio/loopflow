---
asana_id: '1213869424750361'
linear_id: dff685b9-5630-4908-9359-de190ac02a1a
notion_id: 32af8f99-3d81-8194-b75c-c5c50bea610c
---
# PM Provider Auth: OAuth-Only Cleanup

## Problem

Asana and Linear still carry API-key fallback paths alongside OAuth. Notion nominally shipped OAuth-only, but the `configure` surface that stores API-key credentials still accepts it without complaint. The result is a documented invariant — *"PM auth should converge on browser-based OAuth rather than a mix of OAuth and API-key setup paths"* (`wave/pm/README.md`) — that the code contradicts three ways:

- `ops/pm.rs:239` reads `ASANA_ACCESS_TOKEN` / `LINEAR_API_KEY` *before* checking the store, so a stray env var silently wins over the browser-connected credential.
- `lf op auth configure <provider>` stores an API-key credential for any provider, including the three PM ones, with `credential_type: ApiKey`. The README still advertises this path for Linear.
- `resolve_provider_token` returns any stored credential regardless of `credential_type`, so an old API-key row in the DB keeps working indefinitely after we "move to OAuth."

Users pay the cost in support confusion ("my `ASANA_ACCESS_TOKEN` stopped working", "do I need `configure` or `connect`?") and the next wave item — `3-run-lifecycle-sync` — inherits two auth surfaces to reason about instead of one.

## Approach

Delete the PM API-key paths. One resolve route for Asana, Linear, and Notion: stored OAuth credential or a clear error pointing at `lf op auth <provider>`. No silent env-var override. No `configure` command for PM providers. No honoring of `credential_type: ApiKey` rows in the store for PM providers.

Non-PM providers (`github`, `claude`, `codex`, `zen`) keep `configure` — they have legitimate API-key setups — so the change is a narrow gate at the PM boundary, not a global deletion.

Ship this as one coherent change, not a soft deprecation. The style guide is explicit: backwards-compatibility shims for internal config paths are not something we maintain.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|------------------|
| Does Asana's code-paste OAuth (`oob` redirect) actually work today? | Yes — `lf/commands/auth.rs:76-78` prompts the user to paste the code after the browser redirect; token exchange + storage is production-ready. | Removing `ASANA_ACCESS_TOKEN` does not lock out users — they can connect. |
| Do Linear / Notion localhost callbacks work? | Yes — `provider_auth.rs:2281` binds a temporary listener at 19222 (Linear) / 19223 (Notion) for the duration of the flow. | Same as above. |
| Are client IDs/secrets baked into the binary? | **No.** `oauth_client_credentials` (`provider_auth.rs:2268`) requires `<PROVIDER>_CLIENT_ID` / `_CLIENT_SECRET` env vars and errors with `CommandUnavailable` otherwise. | Pre-existing limitation. **Not** what this item solves. The error path surfaces it; the fix is a separate follow-up (default OAuth app + secret broker). We document this plainly so reviewers and users know where the line is. |
| Do tokens expire? Is 401 refresh wired? | Asana issues refresh tokens; a background task (`triggers/token_refresh.rs`) refreshes 20 min before expiry. Linear and Notion do not issue refresh tokens and their tokens do not expire. No 401-replay handler exists in the PM clients. | Not a new problem introduced here. Out of scope. |
| Does Asana's stored token carry workspace context? | No — `asana.rs:108-130` resolves workspace from `asana.workspace` config or single-workspace auto-detect. | Orthogonal to auth surface. Out of scope. |
| Does `configure` reach non-PM providers? | Yes — `github`, `claude`, `codex`, `zen`. | Gate the removal to PM providers, not a blanket deletion. |
| Are there existing users with `ApiKey` rows stored for asana/linear/notion? | Possible — the `configure` path has shipped. Credential rows carry `credential_type`. | `resolve_provider_token` must refuse `ApiKey` rows for PM providers with a migration message, not silently keep using them. |
| What happens if we remove `ASANA_ACCESS_TOKEN` support and nothing is stored? | Today: "No asana credential found. Run `…`." (`ops/pm.rs:256`). | Keep this message; just make the suggested command always be `lf op auth asana` (OAuth), never `configure`. |
| Does `lfq auth` HTTP API expose a `configure` route? | Yes — `configure_credential_handler` in `lfd/http/routes/auth.rs`. | Mirror the CLI gate: reject PM providers at the route level. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Soft deprecation — warn on env-var hit, keep `configure`, schedule removal | Avoids breaking existing setups | Dual paths persist, invariant still violated, the 3-run-lifecycle-sync design still has to reason about both. "Deprecated but supported" is the worst of both. |
| Remove env vars but keep `configure` | One less path | Leaves `configure asana` as a documented way to store API keys — doesn't match the invariant. The README would still have to say "OAuth-only except for this path." |
| Ship default OAuth app + secret broker now | Real turn-key OAuth | Different scope entirely: provisioning an OAuth app per provider + standing up a secret-proxy service. Drags this item into infra territory and delays the invariant we can satisfy today. Follow-up item. |
| Reject `ApiKey` rows by migrating them (re-run OAuth on startup) | Fully automatic migration | Implicit credential mutation on startup is surprising. An explicit error + "run `lf op auth <provider>`" respects user consent and is two lines of code. |

## Key decisions

1. **Hard removal, no grace period.** Env-var consumption in `resolve_provider_token` is deleted for the three PM providers. There is no flag, no warning window.
2. **Scope the `configure` gate to PM providers.** Both the `lf op auth configure` CLI path (`lf/commands/auth.rs:96`) and the HTTP `configure_credential_handler` reject `asana | linear | notion` with an error message. Other providers unaffected.
3. **`credential_type: ApiKey` rows are rejected at resolve time for PM providers**, not at startup. Existing rows stay in the DB (we don't mutate on behalf of the user); they become inert the moment a PM operation tries to use them, and the error tells the user to re-run OAuth.
4. **Error strings name the exact command.** "Asana requires OAuth. Run `lf op auth asana` to connect." — not a generic "auth error." The biggest UX risk of removal is a user hitting a wall; the wall needs a door.
5. **Document the client-credentials limitation explicitly** in `wave/pm/README.md` and the error path. "OAuth-only" today still requires the user to register an OAuth app and set `<PROVIDER>_CLIENT_ID` / `_CLIENT_SECRET`. Pretending otherwise would be dishonest.
6. **Ship the doc updates in the same PR.** README currently advertises `lfq auth linear` as "store Linear API key" and the Python API snippet says the same. Code and docs change together or the drift reopens.

## Scope

- **In scope**
  - Delete `ASANA_ACCESS_TOKEN` / `LINEAR_API_KEY` env-var reads in `ops/pm.rs::resolve_provider_token` (and the corresponding `env_var_for_token` entries if they surface PM providers).
  - Gate `lf op auth configure <pm-provider>` with a clear error.
  - Gate `POST /v0/auth/configure` in `lfd/http/routes/auth.rs` for PM providers.
  - Reject `credential_type: ApiKey` rows at `resolve_provider_token` for PM providers.
  - Update error messages to always suggest `lf op auth <provider>`.
  - Update `README.md` (the `lfq auth linear` / "store API key" language, the PM install snippets), `TESTING.md` if any test references env vars, and `wave/pm/README.md` to state the OAuth-only rule plainly and document the client-credentials caveat.
  - Tests: (a) env var present + nothing stored → error with new message; (b) `ApiKey` row stored for asana/linear/notion → rejected with migration message; (c) `lf op auth configure asana` exits non-zero; (d) existing non-PM `configure` paths still work (smoke).

- **Out of scope**
  - Typed `AuthStep` discriminated union (Swift + Rust).
  - Provider capability declarations (`DeviceCodeStep`, `TerminalStep`, etc.).
  - Provider provenance badges on connected cards.
  - Shipping default OAuth app client credentials / secret broker — separate item.
  - Asana PKCE migration (would let us drop client_secret for Asana specifically).
  - 401-replay / automatic token refresh on PM client calls.
  - Asana workspace / Linear team configuration UX.

## Done when

- `rg ASANA_ACCESS_TOKEN rust/` and `rg LINEAR_API_KEY rust/` return no matches outside the removed/replaced paths.
- `lf op auth configure asana` exits with: `"Asana requires OAuth. Run 'lf op auth asana' to connect."` (same shape for linear, notion).
- `curl -X POST /v0/auth/configure` with provider `asana | linear | notion` returns a 400 with the same message.
- `ops/pm.rs::resolve_provider_token` returns an error when the stored credential for a PM provider has `credential_type: ApiKey`, with text directing the user to re-run `lf op auth <provider>`.
- `cargo test -p loopflow` passes with new tests for the four cases above.
- `README.md` no longer advertises `lfq auth linear` as "store Linear API key" and no install snippet references `ASANA_ACCESS_TOKEN` / `LINEAR_API_KEY`.
- `wave/pm/README.md` has one paragraph stating the OAuth-only rule and the client-credentials caveat.
- A user with an old `ApiKey`-type row for `asana` who runs `lf op pm pull` sees the migration message and — after running `lf op auth asana` — the pull succeeds.

## Measure

Not a performance change. The relevant measurement is code paths removed and error clarity.

- **Before:** `rg -c "ASANA_ACCESS_TOKEN|LINEAR_API_KEY" rust/` — record baseline count.
- **After:** Count drops to zero in `rust/loopflow/src/ops/` and `rust/loopflow/src/lfd/` excluding the doc-string comment in the error message (if any).
- **Resolve paths:** `resolve_provider_token` branch count drops by one (env-var-first branch deleted for PM providers).
- **Docs coherence:** `rg "lfq auth (asana|linear|notion).*(API key|api key)" README.md` returns nothing.

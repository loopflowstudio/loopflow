# PM Auth & Claim Safety

Two tightly related PM fixes shipping on one branch:

1. **PM auth goes OAuth-only.** Delete `ASANA_ACCESS_TOKEN` / `LINEAR_API_KEY` env-var fallbacks and gate the generic `configure` command so it cannot write API-key credentials for Asana, Linear, or Notion.
2. **Claim coordination becomes same-account-safe.** Today two workers sharing one PM account both "win" the claim because verification only checks `assignee == viewer`. Switch to branch-based verification so the claim is a real lock.

They share a branch because they touch the same PM surface and because the follow-up work (`3-run-lifecycle-sync`) is easier when the auth surface and the claim semantics are both settled.

---

## Current state per PM provider

### Auth

| Provider | OAuth path | Legacy API-key path | `configure` accepts it today? |
|----------|-----------|---------------------|-------------------------------|
| **Asana** | Code-paste OOB flow (`lf/commands/auth.rs:76-78`). Works end-to-end. | `ASANA_ACCESS_TOKEN` env var read **before** the store in `ops/pm.rs::resolve_provider_token`. | Yes — generic `configure` writes an `ApiKey` row. |
| **Linear** | Localhost callback on :19222 (`provider_auth.rs:2281`). Works end-to-end. | `LINEAR_API_KEY` env var, same env-var-first ordering. | Yes — generic `configure` writes an `ApiKey` row. |
| **Notion** | Localhost callback on :19223. OAuth-only by design. | None. | Yes — generic `configure` still accepts it despite no fallback existing. |

All three OAuth flows require `<PROVIDER>_CLIENT_ID` / `_CLIENT_SECRET` in the lfd process env (`oauth_client_credentials`, `provider_auth.rs:2263`). These aren't baked into the binary. Model / coding-harness providers (Claude, Codex, Zen) are a separate auth story with a legitimate API-key track — out of scope here.

### Claim coordination (shipped in #614)

`pm_try_claim_async` in `ops/pm.rs:921` walks unassigned items, calls `claim_item`, and verifies. Per-provider:

- **Linear** (`lfd/pm/linear.rs:387`): mutation sets `assigneeId` + `branchName`, verify checks `actual_assignee == viewer_id`.
- **Asana** (`lfd/pm/asana.rs:377`): PUT sets `assignee: "me"`, re-read checks `assignee.gid == my_gid`; also posts a "Working branch:" comment.
- **Notion**: best-effort status flip, no claimant identity, honest about it.

The Linear and Asana verifies pass as long as the assignee is *the token owner*. Two workers sharing one account are both the token owner, so both pass — the check doesn't distinguish workers. Branch names are unique per worker (`{user}.{name}.{timestamp}`), so the branch is the missing discriminator.

---

## Why these can ship now

- OAuth works end-to-end for all three PM providers; Notion already ships OAuth-only as a working precedent.
- Asana issues refresh tokens with proactive refresh (`triggers/token_refresh.rs`); Linear and Notion tokens don't expire.
- The env-var API-key paths are bootstrap residue from #562 that never got deleted when OAuth arrived in #564 — unremoved scaffolding, not an intentional feature.
- The `configure` command reaching PM providers is collateral reach from the model-provider story in #512, not a PM design decision.
- Client-credentials injection (Doppler → `<PROVIDER>_CLIENT_ID`/`_CLIENT_SECRET`) is not solved by this item and is not regressed by it. Operators inject those via `doppler run` or shell profile today; automating it is a separate, bigger item.
- Branch-based claim verification is a five-line fix on Linear and a one-custom-field addition on Asana. No new infrastructure, no per-worker PM identities (which would cost Asana seats).

---

## What to build

### 1. PM auth: OAuth-only

- Delete `ASANA_ACCESS_TOKEN` / `LINEAR_API_KEY` env-var reads in `ops/pm.rs::resolve_provider_token` and the matching `env_var_for_token` entries in `provider_auth.rs:132-133`.
- Gate the CLI: `lf op auth configure <asana|linear|notion>` exits non-zero with `"Asana requires OAuth. Run 'lf op auth asana' to connect."` (same shape per provider). CLI gate lives in `lf/commands/auth.rs`.
- Gate the HTTP route: `POST /v0/auth/configure` in `lfd/http/routes/auth.rs::configure_credential_handler` returns 400 with the same message when the provider is asana/linear/notion.
- Non-PM providers (`github`, `claude`, `codex`, `zen`) remain unaffected — the gate is scoped to the three PM providers, not a global deletion.
- Update error messages in `resolve_provider_token` so the "no credential" path always suggests `lf op auth <provider>`, never `configure`.
- No DB backwards-compatibility work. Stale `ApiKey` rows for PM providers in the store are left alone; no migration, no rejection logic. If they happen to work (Asana PATs and Linear API keys are used the same way as OAuth tokens at the HTTP layer), they keep working. If they don't, the user reconnects.

### 2. Linear claim: verify `branchName`

In `lfd/pm/linear.rs::claim_item`, after the existing mutation:

```rust
let actual_branch = response
    .pointer("/issueUpdate/issue/branchName")
    .and_then(|v| v.as_str());
if actual_branch != Some(branch) {
    return Err(PmError::Message(format!(
        "item claimed by another worker (expected branch {branch}, got {actual_branch:?})"
    )));
}
```

Keep the existing assignee check as a cheap sanity check for auth drift.

Test `claim_item_verifies_assignee_matches_viewer` becomes `claim_item_verifies_branch_matches`. New test `claim_item_fails_same_account_concurrent` — two claim calls with the same auth token but different branches, only one succeeds.

### 3. Asana claim: one shared workspace "Working branch" custom field

- **At init.** `lf op pm init` looks up a workspace-level custom text field named "Working branch". Creates it at the workspace level if missing. Attaches it to the project being initialized if not already attached. Persists the field GID somewhere the provider client can read it at claim time (options: Asana project metadata lookup, or cached in lfd state — open choice during implementation).
- **One field per workspace.** Multiple loopflow projects in the same workspace share the single field. Lookup is by name (or by cached GID if already known).
- **No migration work.** If an Asana project predates this change, `lf op pm init <wave>` gets re-run. That re-init is the migration.
- **At claim.** `claim_item` sets the custom field value to the branch in the same PUT that assigns `"me"`, re-reads the task, verifies `custom_fields[working_branch] == our branch`. Returns a clear error if another worker won.
- **Keep the `"Working branch: \`{branch}\`"` comment.** It's useful as a log / activity trail in Asana's UI. The custom field is the lock; the comment is context.

Test equivalents to Linear: `claim_item_verifies_working_branch_matches` and `claim_item_fails_same_account_concurrent`.

### 4. Notion: no code change, honest docs

Notion's best-effort "In Progress" status flip stays as-is. Document the asymmetry in `wave/pm/README.md` Risks section: Linear and Asana are branch-locked; Notion is best-effort and arbitrated at PR time.

---

## Constraints

- Claim lock depends on branch-name uniqueness per worker. This is true today (`{user}.{name}.{timestamp}`). Document it in `wave/pm/README.md` as load-bearing for same-account claim safety.
- No per-worker PM identities. Asana charges per seat; concurrency is not solved by paying per worker.
- One shared workspace custom field for Asana, not per-project. Share the GID; attach per project.
- Non-PM providers unaffected by the auth gate. Gate is narrow.
- Ship auth and claim fixes in one PR. Different code paths, but both cleanup the PM surface and both need the wave README updated together.

---

## Done when

**Auth:**
- `rg ASANA_ACCESS_TOKEN rust/` and `rg LINEAR_API_KEY rust/` return no matches outside tests that document the removal.
- `lf op auth configure asana` exits non-zero with the specified error; same for linear and notion.
- `POST /v0/auth/configure` with a PM provider returns 400 with the same message.
- `resolve_provider_token` no longer reads PM env vars; its "no credential" error message points at `lf op auth <provider>`.
- Non-PM `configure` paths (github, claude, codex, zen) still work — smoke test.

**Claim:**
- Linear `claim_item` verifies `branchName`. Test: two same-account calls with different branches, one wins.
- Asana `lf op pm init` provisions one shared "Working branch" custom field at the workspace level and attaches it to the project. Re-running `init` on the same project is a no-op.
- Asana `claim_item` writes the custom field, reads it back, verifies. Test: two same-account calls with different branches, one wins.
- Asana `"Working branch: …"` comment is still posted on successful claim.

**Docs:**
- `README.md` no longer advertises `lfq auth linear` as "store Linear API key" and no install snippet references `ASANA_ACCESS_TOKEN` or `LINEAR_API_KEY`.
- `wave/pm/README.md` Risks section states: Linear and Asana are branch-locked; Notion is best-effort; branch-name uniqueness is load-bearing.

**Manual:**
- Run two `lf` processes under one Linear account in parallel on a test wave. Only one picks the item; the other falls through to the next item or reports none available.
- Same test on an Asana-backed wave.

---

## Out of scope

- Typed `AuthStep` discriminated union (Swift + Rust).
- Provider capability declarations (`DeviceCodeStep`, `TerminalStep`, etc.).
- Provider provenance badges on connected cards.
- Default OAuth app client credentials / secret broker — the `<PROVIDER>_CLIENT_ID`/`_CLIENT_SECRET` injection problem is separate and bigger.
- Extending `KEY_MAPPINGS` in `lfd/secrets.rs` to propagate OAuth client credentials from Doppler. Also separate.
- Asana PKCE migration.
- 401-replay / automatic token refresh in PM clients.
- Asana workspace / Linear team configuration UX.
- Per-worker PM identities.
- Notion claim arbitration at the claim layer.

# Provider Elevation

## Problem

Providers today are auth-only — `Provider` enum has GitHub/Claude/Codex, and that's it. There's no way to ask "what models does Claude offer?" or "what does an Opus token cost?" The usage endpoints report model strings (`"opus"`, `"o3"`) but can't contextualize them — no display names, no provider association, no cost rates.

For a solo operator running 10+ waves, the question "am I getting value?" requires knowing not just token volume but what those tokens cost and which models generated them. Provider Elevation makes providers carry their model catalogs and cost rate slots, so downstream consumers (Concerto model picker, cost estimation, `lfq usage --dollars`) have structured data to work with.

## Approach

New `providers` module alongside `provider_auth`. A static model catalog in Rust, merged with live auth status at request time. One new endpoint: `GET /v0/providers`.

### Key types

```rust
// rust/loopflow/src/lfd/providers.rs

#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    pub id: &'static str,           // "claude", "codex", "opencode"
    pub display_name: &'static str, // "Claude", "Codex", "OpenCode"
    pub auth_status: Option<AuthStatusDto>,  // None for providers without managed auth
    pub models: Vec<ModelInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: &'static str,           // "opus", "sonnet", "haiku", "o3", "o4-mini"
    pub display_name: &'static str, // "Claude Opus 4.5", "Codex o3"
    pub is_default: bool,           // true for the model used when no variant specified
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_rates: Option<CostRates>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CostRates {
    pub input_per_mtok: f64,        // $/million input tokens
    pub output_per_mtok: f64,       // $/million output tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_per_mtok: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write_per_mtok: Option<f64>,
}
```

### Static catalog

Hardcoded `MODEL_CATALOG` constant. Three providers, their known models, and cost rates where applicable:

| Provider | Model | Default | Cost rates |
|----------|-------|---------|------------|
| claude | opus | yes | $15/$75 per Mtok |
| claude | sonnet | no | $3/$15 per Mtok |
| claude | haiku | no | $0.80/$4 per Mtok |
| codex | o3 | yes | None (subscription) |
| codex | o4-mini | no | None (subscription) |
| opencode | (provider-dependent) | — | None (user-configured) |

Claude rates are from the Anthropic API pricing. Codex is subscription-only — no per-token cost. OpenCode delegates to whatever provider the user configured, so we list it without models or rates.

### Endpoint

`GET /v0/providers` returns the catalog merged with live auth:

```json
{
  "object": "list",
  "providers": [
    {
      "id": "claude",
      "display_name": "Claude",
      "auth_status": { "status": "active", "login": "jack" },
      "models": [
        {
          "id": "opus",
          "display_name": "Claude Opus 4.5",
          "is_default": true,
          "cost_rates": {
            "input_per_mtok": 15.0,
            "output_per_mtok": 75.0,
            "cache_read_per_mtok": 1.5,
            "cache_write_per_mtok": 18.75
          }
        },
        {
          "id": "sonnet",
          "display_name": "Claude Sonnet 4",
          "is_default": false,
          "cost_rates": {
            "input_per_mtok": 3.0,
            "output_per_mtok": 15.0,
            "cache_read_per_mtok": 0.30,
            "cache_write_per_mtok": 3.75
          }
        }
      ]
    },
    {
      "id": "codex",
      "display_name": "Codex",
      "auth_status": { "status": "none" },
      "models": [
        { "id": "o3", "display_name": "o3", "is_default": true },
        { "id": "o4-mini", "display_name": "o4-mini", "is_default": false }
      ]
    },
    {
      "id": "opencode",
      "display_name": "OpenCode",
      "auth_status": null,
      "models": []
    }
  ]
}
```

### Handler flow

1. Load static catalog (zero-cost, it's `&'static`)
2. Fetch auth snapshots from `ProviderAuth` for Claude and Codex
3. Merge auth into provider entries
4. Return

No database queries. No network calls. Fast.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Extend `Provider` enum with model data | Conflates auth and agent concerns. GitHub is auth-only, OpenCode has no managed auth. Adding OpenCode to Provider breaks the auth contract. | Provider stays auth-scoped. Elevated concept is separate. |
| Config-file model catalog | Flexible, user-editable. But adds config surface area for data that rarely changes. Users who need custom models can use `parse_agent()` freely. | Over-engineering for v0. Models change quarterly, not daily. A code update when models change is fine. |
| Fetch model lists from provider APIs at runtime | Always accurate. But adds network dependency, latency, and failure modes to a read-only catalog endpoint. | The catalog is reference data. Staleness is acceptable; downtime is not. |
| Merge into existing `/v0/auth` endpoint | One fewer endpoint. But changes the auth response contract, and auth clients don't need model data. | Separate concerns, separate endpoints. |

## Key decisions

**Provider vs Harness distinction preserved.** `Provider` (auth) and `HarnessKind` (execution) remain separate. The new `ProviderInfo` bridges them: it uses the harness ID as its `id` field and looks up auth by mapping harness→provider (claude→Claude, codex→Codex, opencode→None). This avoids adding OpenCode to the auth enum or GitHub to the harness enum.

**Cost rates are per-million tokens, not per-token.** Per-token rates ($0.000015) are unreadable. Per-million ($15) matches how Anthropic and OpenAI publish pricing.

**OpenCode has no models in the catalog.** OpenCode proxies to user-configured providers — we don't know which models are available. The entry exists so the UI can show "OpenCode: connected" but the model list is empty. When OpenCode sessions report model names in TurnUsage, those flow through usage aggregation normally.

**`is_default` replaces implicit ordering.** Instead of "the first model in the list is the default," an explicit boolean makes it queryable. Maps directly to `parse_agent()` defaults.

**Static catalog, not traits or registries.** A `fn catalog() -> &'static [ProviderEntry]` function returning a slice of const data. No trait objects, no HashMap lookups, no config loading. The catalog is small (3 providers, ~5 models) and changes infrequently.

## Scope

**In scope:**
- `ProviderInfo`, `ModelInfo`, `CostRates` types in new `providers` module
- Static model catalog with Claude/Codex/OpenCode entries
- `GET /v0/providers` endpoint with auth status merged
- Route registration in HTTP mod
- Tests for catalog completeness and endpoint response shape

**Out of scope:**
- Computed cost from rates × usage (that's downstream analytics)
- Model picker UI in Concerto (Phase 04)
- `lfq providers` CLI command (follow-on)
- Dynamic model discovery from provider APIs
- User-configurable model overrides in config

## Done when

```bash
curl http://localhost:4040/v0/providers | jq '.providers[] | {id, models: [.models[].id]}'
```

Returns:
```json
{"id": "claude", "models": ["opus", "sonnet", "haiku"]}
{"id": "codex", "models": ["o3", "o4-mini"]}
{"id": "opencode", "models": []}
```

This advances the wave's goal: "Elevate Provider into a first-class concept carrying model and metering awareness."

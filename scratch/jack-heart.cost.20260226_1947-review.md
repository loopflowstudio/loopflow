# Provider Elevation — Review

## What was implemented

Static provider catalog (`lfd/providers.rs`) with three providers (Claude, Codex, OpenCode), their models, and per-model cost rate tables for API-billed harnesses. New `GET /v0/providers` endpoint merges the static catalog with live auth status from `ProviderAuthService`.

Separately, OpenCode's `map_turn_usage` now extracts the `model` field from session events, which feeds into usage tracking and enables downstream cost lookups via `lookup_cost_rates(harness, model)`.

### Files changed

| File | What |
|------|------|
| `lfd/providers.rs` (new) | `ProviderInfo`, `ModelInfo`, `CostRates`, `ModelRate` types. Static catalog. `lookup_cost_rates()`. `merge_auth()` for endpoint. 9 tests. |
| `lfd/http/routes/providers.rs` (new) | `list_providers_handler` — reads catalog, merges auth snapshots, returns `ListResponse<ProviderInfoDto>`. |
| `lfd/http/mod.rs` | Route registration for `/providers`. |
| `lfd/http/routes/mod.rs` | Module declaration. |
| `lfd/mod.rs` | Module declaration. |
| `sessions/harness/opencode_mapping.rs` | Extract `model` from turn usage properties. |
| `wave/cost/03-provider-elevation.md` | Deleted — work is done. |

## Key choices

**Rates on harness, not on model.** `ModelRate` lives on `ProviderInfo.model_rates`, not on `ModelInfo`. This separates display concerns (model names, defaults) from cost concerns (rate tables). `lookup_cost_rates(harness, model)` does prefix matching so versioned model IDs (e.g. `kimi-k2-0711`) resolve to the base rate.

**Claude/Codex = subscription, no rates.** The catalog reflects how loopflow users access these providers (subscription plans), not API pricing. `model_rates` is empty for subscription harnesses. Only OpenCode, which proxies to real API-billed providers, carries cost rates.

**Static catalog, not config.** The catalog is `&'static` data — no config files, no runtime loading, no network calls. The provider list changes infrequently; a code update when it does is fine.

**`ListResponse<T>` convention.** During gate, the custom `ProvidersResponse` was replaced with the standard `ListResponse<ProviderInfoDto>` pattern used by all other list endpoints, and `object: "provider"` was added to the DTO.

## How it fits together

```
Static catalog (PROVIDER_CATALOG)
    + live auth snapshots (ProviderAuthService::list_statuses)
    = merged ProviderInfoDto[] → GET /v0/providers

Static catalog (PROVIDER_CATALOG)
    + model string from TurnUsage
    = lookup_cost_rates(harness, model) → Option<CostRates>
```

The endpoint is read-only and fast — no DB queries, no network calls. The cost lookup is a pure function over static data.

## Risks and bottlenecks

**Model name staleness.** Cost rates are hardcoded. If Moonshot or Alibaba change pricing, the rates drift silently. Accepted risk per the design doc — a future wave item could automate drift detection.

**Prefix matching ambiguity.** If two model prefixes overlap (e.g. `qwen3` and `qwen3-coder`), the first match wins. Current entries don't overlap, but future additions need to maintain this invariant. Could add a test.

## What's not included

- Computed cost from rates x usage (downstream analytics, later wave item)
- Cost rates in the endpoint response (rates are available via `lookup_cost_rates`, not serialized to the API)
- `lfq providers` CLI command
- Concerto model picker UI
- Dynamic model discovery from provider APIs

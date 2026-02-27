# 03: Provider Elevation

Extend Provider beyond auth into a concept carrying model awareness and cost rate slots.

## What to build

Static model catalog in `lfd/providers.rs`. Three types:

- `ProviderInfo` — id, display name, models, optional cost rates, is_default, auth provider mapping
- `ModelInfo` — id, display name, is_default
- `CostRates` — input/output/cache-read/cache-write per million tokens

Catalog entries: Claude (with Opus/Sonnet/Haiku models and API pricing), Codex (subscription, no rates), OpenCode (no managed auth, no rates).

New endpoint `GET /v0/providers` merges static catalog with live auth status from `ProviderAuthService`.

## Decisions

- **Gemini excluded** — `parse_agent()` supports it but the catalog doesn't list it. Intentional: catalog tracks providers with managed lifecycle, not all possible harnesses.
- **Naming: `/v0/providers`** — overloads the `Provider` auth enum slightly, but is the right user-facing term.
- **Auth status shape** — new `AuthStatusDto` with just `status` + `login` (drops redundant `provider` field from `AuthProviderStatusDto` since it's nested).
- **Cost rates are Opus-only** — single rate set on the provider, not per-model. Sufficient for initial cost estimation; per-model rates can come later.
- **Model name staleness** — accepted risk. Names/rates are hardcoded. Future wave item: automated scan to detect drift against pricing pages.

## Done when

`curl /v0/providers` returns Claude/Codex/OpenCode with their available models and auth status.

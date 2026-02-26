# 03: Provider Elevation

Extend Provider beyond auth into a concept carrying model awareness and cost rate slots.

## What to build

`ProviderInfo` type combining auth status with available models. `ModelInfo` type with display name, provider, and optional cost rates (None for subscription plans, populated for API-key providers).

New endpoint: `GET /providers` returning provider list with auth status and models.

## Done when

`curl /providers` returns Claude/Codex/OpenCode with their available models and auth status.

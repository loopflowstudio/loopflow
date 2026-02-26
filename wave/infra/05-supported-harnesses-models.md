# 05: Supported Harnesses + Model Resolution

**Status: shipped** (branch `jack-heart.supportedharnessesmodels.20260226_0922`)

Standardized model semantics to `harness:model` and wired one shared resolution chain across CLI, daemon, wave execution, and Concerto.

## Shipped scope

- Renamed parser/variable terminology from `backend/variant` to `harness:model`, then unified the config/API field name to `agent` (with `step_agents` for per-step overrides).
- Made `agent` optional in config and added `supported_harnesses` config with additive merge behavior.
- Added step `default_agent` so step authors can suggest an agent without overriding user preference.
- Updated launch resolution order: explicit > step `agent` requirement > config `agent` > step `default_agent` > fallback `"claude:opus"`.
- Added wave-level `agent` and `step_agents` overrides via PATCH `/v0/waves/:id` and wave config persistence.
- Extended `/v0/flows` metadata with step agent fields and repo `supported_harnesses`.
- Updated LoopflowCore + Concerto with wave agent fields, a model picker in StepRunner, and step override badges.

## Carry-forward follow-ups

1. **PATCH tri-state semantics for agent fields**
   - Current contract cannot distinguish omitted keys from explicit `null`.
   - If clients need explicit-null clearing semantics, move `agent` and `step_agents` request fields to a tri-state payload type.

2. **Per-step override editing in Concerto**
   - Current UI shows per-step agent override badges only.
   - Add an edit surface so users can set/clear `step_agents` directly in flow progress UI.

3. **Concerto macOS UI-test stability**
   - Local `xcodebuild test` still intermittently exits early with `ConcertoUITests-Runner` failures.
   - Verify CI-vs-local setup differences and gate/skip behavior when `lfd` is unavailable.


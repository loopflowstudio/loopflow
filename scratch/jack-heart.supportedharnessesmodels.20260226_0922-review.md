# Review: Supported Harnesses + Model Resolution

## What was implemented

Standardized the agent/model naming from `backend/variant` to `harness:model` across the full stack, then further unified to `agent` as the config/API field name. Added three key capabilities:

1. **`agent` is now optional in config.** Previously `agent_model` defaulted to `"claude:opus"`. Now it's `Option<String>`, enabling step-level `default_agent` to influence model selection without forcing a global override.

2. **`supported_harnesses` config.** A new additive config list exposed via the `/v0/flows` API and surfaced in Concerto's settings and model picker. Lets users declare which harnesses their repo supports.

3. **Wave-level agent overrides.** `agent` and `step_agents` fields on `WaveConfig` (persisted to `wave/<name>/<name>.yaml`) and surfaced via `PATCH /v0/waves/:id`. Concerto's StepRunner shows a model picker when `supported_harnesses` is configured, and FlowProgressPills show per-step override badges.

## Key choices

**Naming: `agent` not `model`.** The string `claude:opus` identifies a harness + model pair — the _agent_ that will run. Using `agent` avoids confusion with LLM model IDs and aligns with the codebase's domain language (agents run steps).

**Resolution chain: explicit > step requirement > config > step default > fallback.** The model resolution order in `prepare_launch_prompt` and `wave_model_override` distinguishes hard step requirements (`agent:` in frontmatter) from soft defaults (`default_agent:`). User config always wins over step defaults.

**Optional config with hardcoded fallback.** `config.agent` is `None` by default. The fallback `"claude:opus"` is applied at the point of use in `launch.rs` and `wave/mod.rs` rather than in the config struct, keeping the distinction between "user set this" and "system default" visible.

**Wave config persistence.** Model overrides are persisted to `wave/<name>/<name>.yaml` alongside flow/area/direction. The update function preserves existing keys — it reads, patches, and writes back rather than serializing the full struct.

## How it fits together

```
CLI -m flag
    └→ prepare_launch_prompt(model: Option<String>)
         └→ model.or(step.agent).or(config.agent).or(step.default_agent).or("claude:opus")
              └→ parse_agent("claude:opus") → ("claude", Some("opus"))
                   └→ build_model_command() dispatches to harness-specific builder
```

For wave execution, `wave_model_override()` checks the wave's YAML config for `step_agents[step_name]` or `agent` before falling into the same resolution chain.

Concerto reads `supported_harnesses` from `/v0/flows`, populates a picker in StepRunner, and sends `agent`/`step_agents` via `PATCH /v0/waves/:id`.

## Risks and bottlenecks

- **Wave config file I/O on every step launch.** `wave_model_override()` reads the wave YAML file synchronously for each step. Fine at current scale, but could become noticeable with many concurrent waves. The read is small and fast, so this is acceptable for now.

- **No schema migration for existing wave YAML files.** Any wave config YAML that used the old `model` / `step_models` keys will silently ignore them. Since this is internal tooling with no external users, this is intentional — per CLAUDE.md, no backwards compatibility shims.

- **`agent_model` → `agent` config rename is breaking.** Existing `~/.lf/config.yaml` and `.lf/config.yaml` files using `agent_model:` will silently lose their setting. No serde alias is provided. This is a deliberate migration — the old key is dead.

## What's not included

- **PATCH tri-state semantics.** Cannot distinguish "key omitted" from "key set to null" in the update payload. Documented as a carry-forward follow-up.
- **Per-step override editing in Concerto.** UI shows override badges but doesn't provide an edit surface for `step_agents` — only the wave-level `agent` picker.
- **Concerto UI test stability.** Known intermittent `xcodebuild test` failures on macOS remain unfixed.

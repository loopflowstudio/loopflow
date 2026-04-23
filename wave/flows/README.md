# Flows

The catalog is the map. Execution is the territory.

## Vision

Loopflow's flow/step model is the composable heart of the system — but as the catalog grew past one head-full, both *authoring* ("what does gate do?", "where is kickoff used?") and *execution* ("which step am I on?", "why did xor pick this path?") got opaque.

This wave owns the flow system as a thing users navigate: the taxonomy, the primitives, the static catalog UI, and — next — the live session view that shows what's running and why.

Not the individual steps (they live in their owning waves), not the daemon executor (that's `lfd`), not wave orchestration. Just the flow/step layer: structure, visibility, and composition primitives.

## Strategy

**Agency-based taxonomy.** Flows and steps organize by agency: `build` (manual, human-driven), `govern` (autonomous, system-driven), `ops` (side-channel utilities). Three categories, not thirteen. Step names are bare (`scan`, not `garden/scan`) because directory layout was a leaky implementation detail, not a semantic prefix.

**Static catalog first, session-state next.** The Concerto Flows tab shows the resolved catalog — builtin + repo overrides, flows expanding into steps (including nested flows, xor branches, loops), and reverse "used by" navigation. That's the reference surface. The runtime overlay — current position, router decisions, per-step status, cold-start recovery — is the follow-on and the higher-leverage piece.

**Composition primitives should match how people think.** `xor(X, silence)` appears three times in four xor uses — that's a `maybe(X)` pattern masquerading as a genuine fork. Introduce `maybe` as a unary primitive; keep `xor` for real either-or branches. Simpler rendering, simpler breadcrumbs, silence sentinel disappears.

**Tune placement in anger.** The reorg moved files; whether each flow lives in the right bucket only becomes clear once flows get invoked in real work. Expect placement adjustments as a continuing concern, not a one-time act.

**One source of truth.** The Rust engine parses flows into a structured model. `lfd` exposes it via `GET /catalog`. Swift consumes the DTO. No parallel YAML parsers. No client-side structural inference beyond the "used by" upward walk.

## Goals

- A user can hold the catalog in their head — or, when they can't, navigate it in Concerto without grepping.
- Running flows are legible: which step, what came before, why a branch was taken, what's deferred.
- Composition primitives are small and honest — `maybe`, `xor`, `loop` — no sentinel-leaf workarounds.
- `.lf/flows/*.yaml` overrides are visible in the catalog without being loud.
- Placement of each built-in flow earns its category through use.

## Risks

- Session-state overlay expands unboundedly (run history? cross-session memory? timeline?) — keep it anchored to *one* live session first.
- `maybe` rollout could break `.lf/flows/*.yaml` that reference `xor(X, silence)` — acceptable churn, but needs clear migration surface.
- Placement tuning without a stop condition turns into bikeshedding — tie each move to a concrete use-site.
- Catalog DTO and the engine's internal flow model drift — only one of them is load-bearing for humans, but Swift uses both via the wire.
- iOS layout for the Flows view has no obvious two-pane analog — defer, don't invent.

## Metrics

- Flows in Concerto catalog match `lf validate` output (target: 100%)
- Time from "what does flow X do?" to visible answer in Concerto (target: <5s, no CLI needed)
- Built-in flows using `xor(_, silence)` after `maybe` ships (target: 0)
- Share of live flow runs where a reviewer can answer "which step is running and what came before?" without checking logs (target: 100% after session-state overlay)

# workflows wave memory

Drives the engine: scheduling, providers, flow execution, mutation, and the governance surfaces that expose all of it coherently.

## Shipped

- **Garden engine pieces** — `vsm/s5-scan`…`vsm/s2-assess` builtins, four `govern-*` flows, `garden/scan` → `garden/assess` → `wave/mutate` → `wave/review`, and `wave/mutate` as a shipped step; algedonic signals route through the attention queue.
- **PM pull/push/export** — works for Asana, Linear, Notion; priority buckets map to provider vocabulary; Notion descriptions sync as pages, Asana preserves markdown via `html_notes`.
- **Operate prompt renamed** — `OPERATE.md` → `LOOPFLOW.md`, `<lf:operate>` → `<lf:loopflow>`, default-on via `--no-loopflow` opt-out; lfd sessions default loopflow guidance on.
- **Vendor skill handoff** — steps became vendor Skills and the assembled prompt retired (a few global-discovery/namespace assumptions still ride on vendor runtime).

## Model

- Engine work and governance UX are tightly coupled — the surface is only as good as the underlying data contracts.
- Governance surfaces (runboard, portfolio, calibration, beat programming, release) share one engine-backed model — no dashboard fork, no UI-only shadow state.
- Loopflow hands off interactive sessions to the vendor; it does not reimplement their chat.
- Out of scope here: embedded-terminal/macOS build polish (→ `desktop`) and root's own morning ritual (→ `root`).

## Next

- `release-infra-and-cron-host` (p0) — shared release cadence, local updater, self-hosted `lfd` cron host, budget guardrails.
- `daily-garden-cycle` (p1) — scheduled garden pass produces reviewable mutation PRs.
- `continuous-build-loop` (p1) — loop-mode waves ingest from PM, ship, and report lifecycle unattended.
- `pm-round-trip` (p2) — dependency + lifecycle sync and reset tooling mirror wave/PR reality.
- `governance-surfaces` (p2) — the Concerto system surfaces read from one model.
- `unify-operate-prompt` (p3) — fold the `loopflow.goal` inline const into `LOOPFLOW_DOC`; settle lfd wire control.
- `remove-directions` (p3) — retire the `direction` wire field once steps are vendor Skills; redistribute its text.

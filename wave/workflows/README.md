# Workflows

The engine. Scheduling, providers, flow execution, mutation, and the governance surfaces that expose all of that coherently.

## Priorities

1. **`daily-garden-cycle`** (p1) — root runs a scheduled garden pass and produces reviewable mutation PRs
2. **`continuous-build-loop`** (p1) — loop-mode waves ingest from PM, ship work, and report lifecycle without human babysitting
3. **`session-input`** (p1) — remote clients can read and steer running sessions without terminal access
4. **`activity-normalization`** (p1) — run, attention, and session signals share one stable model across CLI, daemon, and UI
5. **`pm-round-trip`** (p2) — provider state mirrors wave and PR reality, including dependencies and reset tooling
6. **`chat-session-api`** (p2) — lfd exposes typed, resumable session state desktop and mobile can both consume
7. **`governance-surfaces`** (p2) — runboard, portfolio, calibration, beat programming, and release controls read from one engine-backed model

## Workflows owns

- Runtime and flow engine work in `lfd`, the catalog, and scheduling
- Provider sync, ingest, lifecycle state, and cross-wave mutation
- Governance UX backed by the same engine state: runboard, calibration, portfolio, release

## Not here

- Embedded terminal and native chat polish that primarily change the macOS build experience — `desktop`
- Read-only mobile browsing work — `mobile`
- The root wave's own status ritual and boundary decisions — `root`

## Risks

- Engine work and governance UX are tightly coupled — the surface only gets better if the underlying data contracts get better
- Several tasks unlock each other: PM round-trip and scheduling pressure the build loop; session input unlocks future chat clients
- Scheduled automation is only useful if the outputs stay reviewable and calm

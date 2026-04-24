# Workflows

The engine. Scheduling, providers, flow execution, mutation, and the governance surfaces that expose all of that coherently.

## Tasks

1. **`daily-garden-cycle`** (p1) — root runs a scheduled garden pass and produces reviewable mutation PRs
2. **`continuous-build-loop`** (p1) — loop-mode waves ingest from PM, ship work, and report lifecycle without human babysitting
3. **`pm-round-trip`** (p2) — provider state mirrors wave and PR reality, including dependencies and reset tooling
4. **`chat-session-api`** (p2) — lfd exposes the typed, resumable session API desktop and mobile need
5. **`governance-surfaces`** (p2) — runboard, portfolio, calibration, beat programming, and release controls read from one engine-backed model

## Not here

- Embedded terminal and native chat polish that primarily change the macOS build experience (→ `desktop`)
- Read-only mobile browsing work (→ `mobile`)
- The root wave's own morning ritual and status contract (→ `root`)

## Risks

- Engine work and governance UX are tightly coupled — the surface can only be good if the underlying data contracts are good
- Several tasks unlock each other: PM round-trip and scheduling pressure the build loop; chat-session-api unlocks future chat clients
- Scheduled automation is only useful if the outputs stay reviewable and calm

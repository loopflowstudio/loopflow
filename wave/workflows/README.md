# Workflows

The engine. Scheduling, providers, flow execution, mutation, and the governance surfaces that expose all of that coherently.

## Tasks

1. **`daily-garden-cycle`** (p1) — root runs a scheduled garden pass and produces reviewable mutation PRs
2. **`continuous-build-loop`** (p1) — loop-mode waves ingest from PM, ship work, and report lifecycle without human babysitting
3. **`pm-round-trip`** (p2) — provider state mirrors wave and PR reality, including dependencies and reset tooling
4. **`vendor-session-launch`** (p2) — `lf` launches a new interactive session in the vendor's app / embedded TUI / IDE, config-driven and terminal-first
5. **`governance-surfaces`** (p2) — runboard, portfolio, calibration, beat programming, and release controls read from one engine-backed model

## Not here

- Embedded terminal polish that primarily changes the macOS build experience (→ `desktop`)
- Hosting our own interactive sessions — loopflow hands off to the vendor, it does not reimplement their chat (see `release/unreleased/DECISIONS.md`)
- The root wave's own morning ritual and status contract (→ `root`)

## Risks

- Engine work and governance UX are tightly coupled — the surface can only be good if the underlying data contracts are good
- Several tasks unlock each other: PM round-trip and scheduling pressure the build loop
- `vendor-session-launch` is gated by what each vendor exposes to launch a session — spike the mechanism before building
- Scheduled automation is only useful if the outputs stay reviewable and calm

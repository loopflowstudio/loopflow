# Workflows

The engine. Everything below the Concerto UI layer — the CLI, the daemon, the providers, the flow system, the chord model — composes into one engine that defines how loopflow actually runs.

## Tasks

1. **`daily-garden-cycle`** (p1) — chord observes members and proposes mutation PRs on a schedule. Demo: `lf garden` produces a reviewable PR against this repo
2. **`continuous-build-loop`** (p1) — wave in loop mode ingests from PM, ships PRs, reports lifecycle. Demo: turn on loop, walk away, come back to shipped PRs
3. **`chat-session-api`** (p2) — lfd backend for native chat (typed events + bidi input + resumable stream). Demo: desktop chat, phone picks up mid-turn
4. **`pm-round-trip`** (p2) — PM state mirrors wave/PR reality; dependencies + lifecycle + scripted reset. Demo: add `needs:`, Asana graph updates; reset team, clean rebuild

## Not here

- macOS UI polish (→ desktop)
- iOS read-only view (→ mobile)
- Chord-level governance questions about the root wave itself (→ root)

## Risks

- Four tasks cover a lot of architecture. Each PR will be large — that's the deal with "one task per product experience."
- Inter-task dependencies: `chat-session-api` is infrastructure for desktop and mobile chat; `pm-round-trip` is infrastructure for `continuous-build-loop`.
- Scheduled execution (cron-driven garden, overnight loop) only works if waves and providers stay stable enough — so hardening comes before cadence polish.

# Root

Garden the active waves. Keep the whole system legible.

Root is the conductor wave for this repo. It does not own a product surface of its own; it owns the rhythm between the other waves. The job is simple: keep the active waves moving in the right order, surface drift early, and make the morning status pass feel like one coherent ritual instead of disconnected dashboards.

## Active waves

```
wave: root
│
│  flow: garden
│
├── wave: systems      The operation around the code — CI, releases, automation, self-hosted spine
├── wave: architecture Capability up, weight down; the shape of the code
├── wave: concerto     Concerto macOS — framing the vendors' embedded sessions
├── wave: website      Public site + single-source docs, deployed from this repo
├── wave: workflows    The engine — scheduling, providers, flow execution, governance UX
└── wave: goals        The Goal-driven wave framework itself

(mobile is archived — loopflow ships no mobile surface)
```

## What root is for

- **Garden the other waves** — scheduled `garden` and `govern-*` passes observe pressure, propose mutations, and keep the wave map honest
- **Unify status language** — manual `review-open-work` and automated govern/garden passes should produce the same kinds of signals
- **Keep scope clean** — systems owns the operation (CI, releases, infra), architecture owns the shape of the code, concerto owns Concerto's framing UX, website owns the public story, workflows owns engine + governance machinery, goals owns the wave framework

## Current priorities

1. **Release infra and cron host** — nightly/weekly release cadence, local updater, maintained self-hosted `lfd`, and budget guardrails
2. **`review-open-work-and-garden-parity`** — manual and automated status passes should read as one system
3. **Calendar rhythm that earns trust** — scheduled scans need to be current enough to matter and quiet enough not to create noise
4. **Mutation review that stays human-sized** — root should propose reviewable adjustments, not dump a strategy deck into a PR

## Boundaries

### Root owns

- The relationship between the active waves
- The status vocabulary shared across manual and automated review
- The schedule and posture of garden/govern flows

### Root does not own

- Embedded terminal and chat implementation details — `concerto`
- Flow engine, PM sync, and governance surfaces — `workflows`
- CI, release cadence, local updater, and self-hosted cron deploy details — `systems`
- Code architecture and simplification — `architecture`

## What success looks like

Open Concerto in the morning and get the full picture in one pass: shipped work, blocked work, release health, cron-host health, mutation proposals, calibration checkpoints, and anything that needs a human. Root is doing its job when that ritual feels obvious and the next action is clear.

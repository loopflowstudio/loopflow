# Root

Garden the active waves. Keep the whole system legible.

Root is the conductor wave for this repo. It does not own a product surface of its own; it owns the rhythm between the other waves. The job is simple: keep `desktop`, `mobile`, `workflows`, and `release` moving in the right order, surface drift early, and make the morning status pass feel like one coherent ritual instead of disconnected dashboards.

## Active waves

```
wave: root
│
│  area: wave/desktop/, wave/mobile/, wave/workflows/, wave/release/
│  flow: garden
│
├── wave: desktop      Concerto macOS — embedded terminal build driver, then native chat UX
├── wave: mobile       iOS read surface for remote lfd — waves and roadmap
└── wave: workflows    Engine, providers, flows, governance UX
```

## What root is for

- **Garden the other waves** — scheduled `garden` and `govern-*` passes observe pressure, propose mutations, and keep the wave map honest
- **Unify status language** — manual `review-open-work` and automated govern/garden passes should produce the same kinds of signals
- **Keep scope clean** — desktop owns build-driving UX, mobile owns the read surface, workflows owns engine + governance machinery, release owns cadence + deploy spine

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

- Embedded terminal and chat implementation details — `desktop`
- Remote iOS read-surface work — `mobile`
- Flow engine, PM sync, and governance surfaces — `workflows`
- Release cadence, local updater, and self-hosted cron deploy details — `release`

## What success looks like

Open Concerto in the morning and get the full picture in one pass: shipped work, blocked work, release health, cron-host health, mutation proposals, calibration checkpoints, and anything that needs a human. Root is doing its job when that ritual feels obvious and the next action is clear.

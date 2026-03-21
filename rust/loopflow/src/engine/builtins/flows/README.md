# Built-in Flows

Flows shipped with loopflow. Organized by purpose.

## Code flows (`code/`)

Inner loops — composable building blocks that don't land.

| Flow | Steps | Use case |
|------|-------|----------|
| `code` | implement → compress → lint → gate | Headless build cycle |
| `pair` | design → code | Interactive design then build |
| `deploy` | gate → op: land → op: pm push-diff | Gate, land PR, push PM changes |
| `sync` | rebase → integrate-upstream → op: pm pull | Rebase and pull fresh PM state |

## Build flows (`build/`)

Full cycles that end with deploy.

| Flow | Steps | Use case |
|------|-------|----------|
| `build` | kickoff → review-design → loop(code → xor(demo, code-review), exit: gate) → deploy | Full iterative build cycle |
| `build-or-silent` | ingest → xor(build, silence) | Pick item or stay quiet |
| `s1-build` | kickoff → code → deploy | Autonomous build — no reviews |
| `design-and-ship` | design → implement → reduce → polish → deploy | Design through to ship |
| `queue` | gate → update-wave → deploy | Gate, reconcile wave, then ship |

## Garden flows (`garden/`)

Chord-level scanning, assessment, and mutation of member waves.

| Flow | Steps | Use case |
|------|-------|----------|
| `garden` | garden/scan → garden/assess → xor(act, silence) | Full garden cycle |
| `garden-act` | wave/mutate → wave/review | Apply and review wave mutations |

## Algedonic flows (`algedonic/`)

Urgent signals that bypass normal flow.

| Flow | Steps | Use case |
|------|-------|----------|
| `incident` | debug → 5whys → code → deploy | Fix bug, root cause, ship |

## VSM flows (`vsm/`)

Viable system model governance — scan a specific system dimension and mutate.

| Flow | Steps | Use case |
|------|-------|----------|
| `govern-operations` | ingest → xor(s1-build, silence) | S1 — autonomous build |
| `govern-coordination` | s2-scan → s2-assess → wave/mutate | S2 — coordination risk and interference |
| `govern-control` | s3-scan → s3-assess → wave/mutate | S3 — control health and capacity |
| `govern-intelligence` | s4-scan → s4-assess → wave/mutate | S4 — environmental change response |
| `govern-identity` | s5-scan → s5-assess → wave/mutate | S5 — identity and structural drift |

## Ops flows (`ops/`)

| Flow | Steps | Use case |
|------|-------|----------|
| `release` | op: release run patch | Cut a release |

## Adding a flow

1. Create `{category}/{name}.yaml` with step list
2. Update this README

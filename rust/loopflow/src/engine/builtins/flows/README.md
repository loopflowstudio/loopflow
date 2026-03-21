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
| `build-or-silent` | op: pm pull → ingest → xor(build, silence) | Pull PM, pick item or stay quiet |
| `design-and-ship` | design → implement → reduce → polish → deploy | Design through to ship |
| `queue` | gate → update-wave → deploy | Gate, reconcile wave, then ship |

## Garden flows (`garden/`)

Chord-level scanning, assessment, and mutation of member waves.

| Flow | Steps | Use case |
|------|-------|----------|
| `garden` | garden/scan → garden/assess → xor(garden-act, silence) | Scan, assess, mutate if needed |
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
| `govern-operations` | ingest → xor(s1-build, silence) | Pick operational item and build |
| `govern-identity` | s5-scan → s5-assess → wave/mutate | Identity and structural drift |
| `govern-intelligence` | s4-scan → s4-assess → wave/mutate | Environmental change response |
| `govern-control` | s3-scan → s3-assess → wave/mutate | Control health and capacity |
| `govern-coordination` | s2-scan → s2-assess → wave/mutate | Coordination risk and interference |

## Ops flows (`ops/`)

| Flow | Steps | Use case |
|------|-------|----------|
| `release` | op: release run patch | Cut a release |

## Adding a flow

1. Create `{category}/{name}.yaml` with step list
2. Update this README

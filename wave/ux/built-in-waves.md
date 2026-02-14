---
status: proposed
---

# Built-in Waves

Loopflow ships built-in steps and flows. Built-in waves are the next level — pre-configured wave definitions that users can activate.

## Concept

A built-in wave is a YAML file that describes a complete wave: flow, area, and stimulus. It's a definition, not an active instance. Users browse available waves in Concerto and activate the ones they want. `lfd` only runs activated waves.

```yaml
# builtins/waves/scan.yaml
name: scan
flow: scan
area: [.]
stimulus:
  kind: cron
  cron: "0 8 * * *"
```

## Lifecycle

1. Built-in wave definitions ship with loopflow (embedded in binary, like steps/flows)
2. Concerto shows available waves — built-in + repo-local (`.lf/waves/`)
3. User imports/activates selected waves
4. Activation creates a real wave + stimulus in the database
5. `lfd` runs only activated waves

## What makes built-in waves different from wave specs

Wave specs (see `wave-specs-launcher.md`) are work items with execution metadata — they describe a piece of work to complete, like a roadmap item. Built-in waves are ongoing operational concerns — they run forever, like a cron job. A scan wave doesn't have a "done" state.

Both share the wave YAML format. The difference is intent:
- **Wave spec**: "implement this feature" → completes and stops
- **Built-in wave**: "scan dependencies daily" → runs indefinitely

## First built-in wave: `scan`

Three steps that look outward instead of inward:
- `scan/cves` — check dependencies for known vulnerabilities
- `scan/deps` — check for major version bumps, deprecations, end-of-life
- `scan/upstream` — check external APIs for breaking changes

These exist today as built-in steps and flow. The wave definition exists but activation via Concerto is not yet implemented.

## What to build

### Concerto: available waves view

Show built-in and repo-local wave definitions alongside active waves. Let users activate with one click.

### `GET /waves/available`

Return wave definitions from builtins + `.lf/waves/*.yaml`, cross-referenced with active waves to show which are already running.

### Wave definition parsing

Parse the wave YAML format into a struct that can create a Wave + Stimulus in the database on activation.

## Done when

- `GET /waves/available` returns built-in wave definitions
- Concerto shows available waves with activate/deactivate
- Activating a wave creates it in the database with its stimulus
- `lf scan/cves` works out of the box in any repo

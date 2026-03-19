# 02a: Worker Pools

**Finish line:** Waves have a `workers: N` config that controls concurrent run capacity. `dispatch_pending_activations()` respects the limit. `serialized: bool` is deprecated. No unlimited mode — every wave has a finite worker count.

## Context

Waves have `serialized: bool`. When true, one run at a time — activations queue. When false (default), triggers spawn runs immediately with no limit. The dispatch logic in `activation.rs` already handles both paths.

Worker pools generalize this: `workers: u32` replaces the boolean with a capacity. Default `workers: 1`. This is general-purpose infrastructure — VSM's s1 wave is the motivating case but any wave can use it.

## What to build

Replace `serialized: bool` with `workers: u32`.

| `workers` | Behavior | Equivalent to |
|-----------|----------|---------------|
| `1` | One run at a time, queue the rest | `serialized: true` |
| `N` | Up to N concurrent runs | new |

No `0` / unlimited mode. Every wave has a finite worker count.

```yaml
flow: ship-wave
workers: 3          # up to 3 concurrent runs
```

### Dispatch changes

`dispatch_pending_activations()` counts active runs vs `workers` limit instead of checking a boolean. `spawn_immediate_activation()` checks capacity before spawning — if full, enqueues instead.

### Composition with modes

`workers` composes with any wave mode:

- `flow` + `workers: 3` — triggered batch of up to 3
- `loop` + `workers: 3` — 3 persistent loopers
- `cron` + `workers: 3` — on schedule, launch up to 3

### Migration

`serialized: true` → `workers: 1`. `serialized: false` or absent → `workers: 1` (new default). Deprecate `serialized`, read for backwards compat, remove later.

## Done when

- `workers: N` in wave YAML controls concurrent run capacity
- Default is `workers: 1` (no unlimited mode)
- Dispatch respects the limit: excess activations queue
- `serialized: true` still works (maps to `workers: 1`)
- A wave with `workers: 3` runs up to 3 concurrent activations

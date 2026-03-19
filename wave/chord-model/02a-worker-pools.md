# 02a: Worker Pools

**Finish line:** Waves have a `workers: N` config that controls concurrent run capacity. `dispatch_pending_activations()` respects the limit. `serialized: bool` is deprecated.

## Context

Waves have `serialized: bool`. When true, one run at a time — activations queue. When false (default), triggers spawn runs immediately with no limit. The dispatch logic in `activation.rs` already handles both paths.

Worker pools generalize this: `workers: u32` replaces the boolean with a capacity. This is general-purpose infrastructure that any wave can use — VSM's s1 wave is the motivating case but not the only one.

## What to build

Replace `serialized: bool` with `workers: u32`.

| `workers` | Behavior | Equivalent to |
|-----------|----------|---------------|
| `1` | One run at a time, queue the rest | `serialized: true` |
| `N` | Up to N concurrent runs | new |
| `0` or omitted | Unlimited (current default) | `serialized: false` |

```yaml
flow: ship-wave
workers: 3          # up to 3 concurrent runs
```

Dispatch changes are small: `dispatch_pending_activations()` counts active runs vs `workers` limit instead of checking a boolean. `spawn_immediate_activation()` checks capacity before spawning — if full, enqueues instead.

Deprecate `serialized`, read it for backwards compat, remove later.

## Done when

- `workers: N` in wave YAML controls concurrent run capacity
- Dispatch respects the limit: excess activations queue
- `serialized: true` still works (maps to `workers: 1`)
- A wave with `workers: 3` runs up to 3 concurrent activations

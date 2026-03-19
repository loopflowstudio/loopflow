---
asana_id: '1213718081081138'
linear_id: 70cde070-1b10-4e97-87b0-e72d35e50d7d
---
# Worker Pools + VSM as Chord

## Worker pools

### What exists

Waves have `serialized: bool`. When true, one run at a time. When false (default), unlimited. The dispatch logic in `activation.rs` handles both paths.

### The change

Replace `serialized: bool` with `workers: u32`.

| `workers` | Behavior | Maps from |
|-----------|----------|-----------|
| `1` | One run at a time, queue the rest | `serialized: true` |
| `N` | Up to N concurrent runs | new |
| `0` or omitted | Unlimited | `serialized: false` |

```yaml
flow: ship-wave
workers: 3
```

Dispatch: count active runs vs `workers` limit instead of checking a boolean. Deprecate `serialized`, read for backwards compat.

## VSM as a chord of five waves

Five waves, each with its own flow and rhythm. Not a sequential flow.

```
wave/redesign/
  redesign.yaml              # chord config
  wave/s5-policy/            # identity, boundaries, roster
  wave/s4-intelligence/      # environment scanning
  wave/s3-control/           # resource allocation, health
  wave/s2-coordination/      # deconfliction, queue ordering
  wave/s1-operations/        # worker pool
```

### s5–s2: governance waves

Each wave runs its own flow. The flows might be single-step (`[vsm/s5]`) or multi-step — that's up to the flow definition. What matters is that each wave has independent rhythm:

- **s5** — weekly or slower. Identity doesn't shift fast.
- **s4** — daily. Environment changes constantly.
- **s3** — every few hours. Tight control loop.
- **s2** — before each s1 batch, or on its own cadence.

They read each other's latest output when they run, but they're not trigger-chained into a cascade. s4 doesn't wait for s5. s3 doesn't wait for s4. Each runs on its own clock and works with whatever's current.

All four only edit wave space — plans, backlogs, configs, `workers` on s1. No code changes.

### s1: the worker pool

A wave with `workers: N` and `flow: ship-roadmap`. Workers pull from the backlog (maintained by s2), each in its own worktree. Ephemeral — worktree pruned after landing.

s3 adjusts `workers` on s1 as part of its assessment output. This is a wave config write, same pattern as chord mutations in `draft-chord`.

### Concurrent ingest — exploratory

With `workers: N`, multiple workers call `ingest` simultaneously. Directions worth exploring:

**PM provider as arbiter.** `ingest` already talks to Linear/Asana. The PM provider handles assignment atomically — "assign this to run X" either succeeds or someone else got it. Keeps race resolution out of loopflow. Needs validation: do Linear/Asana APIs actually give us atomic claim semantics?

**Frontmatter status.** Items get `status: available | in-progress | done` in frontmatter. Small race window, not catastrophic — worst case two workers start the same item.

**lfd coordination.** `ingest` calls lfd to claim. Most correct, adds API surface.

Likely some combination. PM provider for happy path, frontmatter as local state. Worth prototyping.

## What this simplifies

- No special "s1 launches subwave runs" machinery. s1 is just a wave with a pool.
- Worker pools are general-purpose — any wave can use `workers: N`.
- Governance levels evolve cadence independently.
- Tend flow still works for human check-ins alongside VSM.

## Ordering

1. **Worker pools** — `workers: u32` replacing `serialized: bool`. General infrastructure, ships independently.
2. **Concurrent ingest** — explore PM provider path for safe multi-worker item claiming.
3. **VSM wave configs** — chord structure with five member waves.
4. **Governance flows** — s5–s2 flow definitions (may be single-step each).
5. **Remove vsm.yaml sequential flow** — replaced by the chord.

## Validation

- `cargo test --test flow_tests builtin_vsm_flow_structure -- --exact`
- `uv run pytest python/tests/test_bootstrap_redesign_script.py -q`
- Full suite: `cargo fmt --check && cargo clippy -- -D warnings && cargo test --all && uv run pytest python/tests/`

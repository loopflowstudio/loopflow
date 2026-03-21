# 02e: Wave Crons — Multiple Scheduled Flows per Wave

**Finish line:** A wave can have a list of cron'd flows alongside its primary flow. Workers run the primary flow. Crons fire supplementary flows on schedule. Root waves default to `workers: 0` with governance crons.

## Context

Currently a wave has one `flow` and one `mode`. Tempo (polish weekly, reduce monthly, governance daily) requires either separate waves per scheduled flow or the ability to attach multiple flows to one wave.

Separate waves per flow fragments identity — a wave's area, direction, and README get duplicated across `my-wave`, `my-wave-polish`, `my-wave-reduce`. The wave *is* the scope; the crons are maintenance rhythms layered on top.

Depends on:
- 02a (worker pools — `workers: N`)

## The change

Wave config gains `crons`: a list of flow + schedule pairs.

```yaml
# member wave — workers grind build, polish sweeps weekly
flow: build
workers: 2
mode: loop
crons:
  - flow: wave-polish
    schedule: "0 0 * * 1"
  - flow: wave-reduce
    schedule: "0 0 1 * *"

# root wave — no workers, governance on crons
flow: garden-or-silent
workers: 0
mode: loop
crons:
  - flow: govern-identity
    schedule: "0 0 * * 0"
  - flow: govern-coordination
    schedule: "0 0 * * *"
  - flow: integrate
    schedule: "0 */6 * * *"
```

`flow` + `workers` + `mode` = the primary work. What the wave *does*.

`crons` = supplementary flows that fire on schedule. Each runs once when triggered, not in the worker pool.

### Interaction with 02c

02c models VSM governance as five separate waves, each with `mode: cron`. With wave crons, the governance flows could instead be crons on the root wave itself — fewer waves, same behavior. Either model works; wave crons make the root wave self-contained while separate waves give each governance level its own identity and backlog.

The two approaches aren't exclusive. A chord might use wave crons for lightweight rhythms (polish, reduce) and separate member waves for heavyweight concerns (s1 worker pool with its own backlog).

### Data model

Rust:
```rust
pub struct WaveCron {
    pub flow: String,
    pub schedule: String,  // cron expression
}

// Wave gains:
pub crons: Vec<WaveCron>,
```

Python:
```python
@dataclass
class WaveCron:
    flow: str
    schedule: str

# Wave gains:
crons: list[WaveCron]
```

Swift (LoopflowCore):
```swift
public struct WaveCron: Codable, Sendable, Equatable {
    public let flow: String
    public let schedule: String
}

// Wave gains:
public var crons: [WaveCron]
```

### Dispatch

`lfd` cron scheduler checks each wave's `crons` list. When a cron fires, it creates a run with that flow, independent of the worker pool. Cron runs don't count against `workers` capacity.

## Done when

- `crons` field on wave config (YAML, Rust, Python, Swift models)
- `lfd` fires cron'd flows on schedule
- Cron runs are independent of worker pool capacity
- Root wave with `workers: 0` + governance crons works
- Member wave with `workers: N` + polish/reduce crons works

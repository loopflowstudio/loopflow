# Signals

The nervous system. Default state is running. When something blocks progress, the system tries to unblock itself first. What it can't resolve propagates up — wave to chord to human.

## Strategy

Start with the block types that actually occur during this redesign. Don't design a taxonomy speculatively — let real blocks define the categories. CI failure and merge conflict exist already. Stall detection and shallow work detection are the high-value additions.

The self-healing cascade (wave tries → chord tries → human decides) is the core architecture. Build it for one block type, then extend.

## Goals

- Block types defined by what actually blocks work, not speculation
- Self-healing cascade: wave → chord → human, with each level trying before escalating
- Stall detection: wave running but not producing
- Shallow work detection: PRs landing but quality is thin
- Human-system drift detection: human approvals getting mechanical
- Tool/capability gap detection: wave shipping code without validating user experience

## Risks

- False positives in stall/shallow detection could erode trust in the system
- Defining "shallow work" is inherently subjective — needs human calibration
- Self-healing could mask problems that should surface earlier

## Metrics

- False positive rate on stall detection (target: <20%)
- Time from block occurrence to resolution (self-healed or human-resolved)
- Percentage of blocks self-healed vs escalated (healthy ratio TBD empirically)

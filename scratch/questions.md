# Assumptions — PRD-39

## The named PRD-20 5 Whys artifact is not present under PRD-20's current description

The cache identifies PRD-20 as “Make LLM-session checkpoints authoritative for
Task landing,” and its branch has no surviving 5 Whys document. Its durable
Task events do contain the route-recovery incident this directive describes:
three repeated Claude-pool failures, a manual handoff to Codex, and a later
mixed missing-credential/cooling failure. Merged PR #1080's deleted
`scratch/resilience.md` contains the corresponding transient-recovery design.

Proceeding with those two records as the intended causal input. The design does
not import PRD-20's unrelated shipping-lifecycle contract.

## PRD-38's interface is still moving

The active PRD-38 worktree has proved sequential Launches and stale-lease
rejection, but is dirty and not landed. Its current `LaunchEnded` then
`rotate_run_lease` sequence is not atomic, and its side-effect evidence is a
boolean. PRD-39 names the required semantics rather than depending on those
temporary symbols. Pure route policy can proceed on main; executor wiring waits
for the landed PRD-38 shape and then rebases through `lf`.

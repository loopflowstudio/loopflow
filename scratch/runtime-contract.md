# Shared transition implementation contract

Implementation choice for the accepted loopflow goal. Main owns generic CLI
execution, persistence, decision command, Task integration, exports, and tests.
A bounded contribution owns engine/transitions.rs only.

Keep Flow/ConcreteStep and the existing RepeatPolicy { from, max_iterations }
authoring shape for now. The edge belongs to its deciding occurrence; from is
the earlier target. Default forward progression requires no verdict. A repeat
occurrence requires a typed verdict; human approval and model decisions keep
their separate authority at the adapters.

Shared engine API (implement in engine/transitions.rs):

```rust
pub enum FlowDecision { Next, Repeat, Blocked } // serde snake_case + clap ValueEnum
pub struct FlowVerdict { pub decision: FlowDecision, pub summary: String }
pub struct FlowProgress {
    pub repeats: BTreeMap<String, u32>, // times each backward edge has been taken
    pub direction: Option<String>,
    pub verdict: Option<FlowVerdict>, // decision for current deciding occurrence
}
pub enum FlowTransition { Next(usize), Repeat(usize), Finished, Blocked(String) }
pub fn finish_step(steps: &[ConcreteStep], index: usize,
                   progress: &mut FlowProgress) -> anyhow::Result<FlowTransition>;
```

FlowProgress is execution state, not a wire DTO. Default is empty. All public
types Debug, Clone as appropriate, serde on persisted state/decision. Use pure
transition computation; no I/O, runtime, provider, database, or second scheduler.
The caller settles returned state under its exact execution authority.

For Next, clear pending verdict/direction and increment cursor (or Finished at
end). For Repeat, require declared earlier target, increment that edge's count,
carry summary as direction, clear pending verdict, and return Repeat(target).
max_iterations bounds initial pass plus backward traversals per edge over the
invocation: refuse Repeat when another traversal would reach max_iterations.
Keep counts across forward progress; this also bounds overlapping cycles. A
decision Blocked and exhaustion return explicit Blocked; missing or empty
decision/evidence is also a visible Blocked. Do not change progress on blocked
outcomes. Invalid cursor/definition are errors.

Main will rename durable LoopReview to shared FlowProgress and translate legacy
review_json objects in its reader so existing execution state is retained.
Main will remove RepeatPolicy validation prohibitions on human/operation bodies
and nested edges once shared per-edge progress is integrated. Edge targets stay
within their expanded execution body; xor child bodies have their own cursors.

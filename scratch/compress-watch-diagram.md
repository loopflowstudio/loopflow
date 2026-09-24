# Connected Watch diagram compression — 2026-09-24

No further coherent model/API reduction found in HEAD `dd88a9968` plus the
working diagram. This pass adds only this report; existing implementation,
tests, documentation and evidence remain intact.

## Model before and after

Unchanged: FlowPosition owns execution. Transactional Task events retain the
expanded plan, attempts, transitions and settlement. Rust projects those facts
and attributed Run manifests into TaskWatchSnapshot; TaskOutputPage separately
pages normalized journal/native output with source evidence. RegistryQuery
transports both. Navigation retains TaskWatchStore; mounted views own cancellable
reads. The diagram renders snapshot stages and transitions directly, measuring
native button geometry without another graph model. TaskWatchOutput merges
loaded revisions and derives display rows.

No command, DTO, field, type, persistence path or API was removed in this pass.
The implementation already deleted the old stage List and its active-stage helper.

## Path and mirrors inspected

Read `work/task/flow_history.rs`, the transactional history read and receipt
writer in `store/sqlite/durable.rs`, Watch projection and output read envelopes.
Compared Rust/Swift Watch and output envelope fields, both shared JSON fixtures,
and their Rust/Swift assertions. Followed CLI declarations and shared read-only
Task lookup through RegistryQuery's typed reads and temporary cursor transport.
Traced WorkspaceNavigation and SessionsView into TaskWatchStore, TaskWatchView,
TaskWatchDiagram and the output model/view. Read the new diagram behavior test,
active design and README delta. Searches found no remaining TaskOutputRecord,
TaskWatchStepKind or old recorded-stage List in the inspected source paths.

## Candidates retained

- **Focus, selection and geometry:** native keyboard focus is transient;
  retained stage selection also changes through Follow and history inspection.
  Anchors describe measured button bounds, not a copied plan. Combining these
  would couple independent lifetimes or replace native geometry with assumptions.
- **Plan order and recorded edges:** planned adjacency includes unentered stages;
  transitions retain actual Iterate targets and retries. Neither derives the
  other. One inferred connector list would discard that distinction.
- **Diagram and attempt detail:** nodes summarize stages and expose return links;
  detail preserves each attempt, readiness, failure and full transition coordinates.
  Repeated transitions can share geometry without losing their textual evidence.
  Extracting shared icon/link spelling would move code without reducing ownership.
- **Stage and attempt identity:** diagram selection covers every attempt at an
  invocation/step. TaskFlowStage also includes iteration. Replacing selection
  with that coordinate would incorrectly narrow its meaning.
- **Snapshot and accumulated output:** auxiliary Runs need no stage; bound
  attempts can lack readable manifests. History/live continuation and revision
  precedence remain independent of snapshot refresh. Combining these inventories
  or deleting source evidence would lose behavior, not simplify its authority.

## Verification and remaining work

No tests or builds rerun because executable content is unchanged. Inspected the
existing [three-test receipt](watch-diagram-evidence/tests.log), including two
render cases and Mac compilation. Diagram, Watch view, test and receipt hashes
match the supplied workspace snapshot. This is prior implementation evidence,
not fresh configured interaction proof. `git diff --check` passes.

Bounded discovery/initialization/retention, visible polling, contiguous cross-Run
observation ordering, complete capture, exact checkpoint Session navigation and
the configured human demo remain required in this Task/PR. Nothing was published,
landed or marked complete.

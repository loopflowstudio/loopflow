# Work model — reviewed direction

2026-10-10 · LOO-412 · Jack Heart accepted the structural reduction in
[One Work model](work-sync-redesign.html). The [deletion audit and queued pass](work-model-reduction.md)
now supplies the exhaustive implementation map. Finish the existing passes in
[the pursuit design](work-on-another-machine-name.md) before taking that pass.
Implementation owns the exact `WorkEdit` shape; no further shape approval is needed.

## One model, explicit boundaries

Wave is the durable workstream; Project owns a body of work, KRs, targets and its
Tasks; Task owns a concrete outcome and its comments. `WorkRef` names these three
scopes. Comments, criteria and provider observations are not additional Work scopes.
Chapter is a convention across Projects, not a low-level object or table.

Shared fields are saved locally and exchanged by the repository's single selected
transport: default user-keyed Git ref, shared Git ref, or Linear. Each Machine can
read/write Linear directly. Disconnection retains local edits; it does not select
another transport. Repository configuration can establish a company's choice.
No master, host callback, shared resident, automatic turn retry or Flow retry.

Only shared Work fields travel. Checkouts, PR delivery, captured Workflow and its
position/history, Sessions, Processes, controls and effect receipts stay local.
A Task struct containing local fields must never be exported wholesale. Receiving
completion changes the shared view; it cannot advance another Machine's Workflow,
prove a provider exited or authorize cleanup. Local absence cannot prove peer absence.

## Accepted reductions

- Put Project content directly on Project: name, summary/authored description,
  status, optional workflow choice, `ProjectKR`s and `MetricTarget`s. Store typed
  criteria once. Render prompts and Linear Markdown only at their boundaries;
  preserve unrelated prose. Remove public ProjectPlan/ProjectContent and ordinary
  PmProject reads. Private Linear response shapes may remain in the adapter.
- Put Task identifier, title and description directly on Task; delete TaskPlan.
  Keep write revisions, timestamps, observation age and provider mappings under
  their actual owners. Do not change legacy Task identities during cleanup.
- Remove live Project iteration; inspection found no production increment.
  Preserve historical iteration events and stored evidence.
- Replace PM with Linear where it means that integration. Wave binds an Initiative;
  repository config binds a Team. Remove duplicate provider selection. Domain
  types remain Work in both Git and Linear modes.
- Use `ProjectKR` and `MetricTarget`; Chapter does not belong in these types.
  `Threshold` can name the target's at-least/at-most comparison.
- Use domain TaskComment and a TaskComments thread/status reading. Linear comment
  and issue observation shapes stay private. Preserve author IDs, revisions and
  direction provenance as integration evidence.
- Remaining work is an ordinary Task with a linking comment. Remove active
  FollowThroughIntent/Link/Projection, the mandatory `--finish`/`--none` ritual,
  special completion gate and dedicated UI lifecycle. Keep historical events and
  uncertain effects. A saved Task survives a failed linking comment; retry only
  the comment. Ordinary process/merge/checkout preservation still applies.
- One cron list contains ordinary commands with `--wave`/`--task` as needed;
  UI scope filtering uses the command parser. Remove Wave-specific cron definitions
  and duplicate interpretation. Preserve schedule identity, timing, receipts,
  Machine ownership and due accounting. No live activation is authorized here.
- Ordinary readers and one typed transactional writer operate on Work. Delete
  parallel PM/Planning hierarchies, dynamic whole-model JSON patching and redundant
  edit schemas. Retain necessary Linear observations and Git causal/delivery
  evidence; never discard foreign/unmapped facts or uncertain writes for tidiness.

## Current source and evidence

The existing pass removed plan wrappers and confined Linear comment observations
(`28913feeda`, `105e1d15ad`). Project persistence also replaces internal Markdown with typed description,
workflow, KRs and targets, including transactional conversion and peer fidelity
proof. Public ProjectContent and its operation arguments are also removed; ProjectStatus
is domain-owned. Linear/migration Markdown stays private; retained rotation receipts
keep their original bytes. Canonical Project operations, rotation and CLI/Desktop summaries now read Work;
the ProjectPlanning wrapper and public PM-shaped Project reads are removed. Shared
rank survives the reader cut. Provider/Task snapshot projections and Linear prose
acquisition remain. This is partial implementation, not completion of the accepted model. The new audit identifies remaining storage, acquisition, Git,
CLI/Desktop, config, skills and test cuts against the merged October 10 source.
Its conditional deletions require behavioral proof before removing their evidence.

The original full inventory and review sequence remain in
[work-model.html](work-model.html), pinned to `c8fe4628affc13ac0553c14cd5bec5e5896369a1`.
Its earlier “proposal” labels and TaskBrief suggestion are superseded by the
accepted direction above. This concise account replaces duplicated historical
inventory in launch context; the HTML retains the original detailed type map.
The initial Markdown also survives in `/tmp/loo412-review-before-pursuit/work-model.md`
as a local recovery copy, not the ongoing design owner.

# Task progression and autonomous execution vocabulary

LOO-386 · Implementation authorized by Jack Heart · October 7, 2026

## Vocabulary and owner-scoped commands

Jack Heart accepted retaining Workflow and Flow with Workflow commands under
Task on October 7, then clarified that Projects and Tasks both have workflows.
Jack proposed no standalone `lf workflow` commands: operations instead belong
under their owner, such as `lf project workflow set` and
`lf task workflow restart`. Jack subsequently clarified that restart resets the
position. Jack then requested moving the work forward into implementation.
Resolve the remaining command mechanics within this owner-scoped direction;
the implementation choices below are not separate quotations or approvals.

A Project selects the Workflow definition its Tasks take up. A Task holds its
captured Workflow and position. Flow stays the independently executable layer,
with `lf flow` and `lf run`. There is no new outer noun or name-driven schema
migration. Project selection changes do not silently replace existing Task
Workflows. Jack's request authorizes implementing this design through pursue
and returning for demo review.

The outcome is to distinguish a Task's Workflow from the autonomous work it
invokes, select either definition without name collisions, and inspect execution
without learning another lifecycle. CLI, code, wire, Swift and documentation
change together. The next authored edge is pursue, followed by demo review.

## Accepted constraints and integrated baseline

Jack Heart accepted retaining `customize` on October 7 after its behavior was
explained: copy a builtin definition into the repository when needed and print
the local path; reuse an existing local file. It does not open an editor. This
acceptance concerns its behavior; placement follows the owner-scoped design.

Jack Heart's October 6 LOO-386 steers retain these decisions:

- The outer graph has nodes and edges. A Project supplies its definition; a
  Task's own selection wins. Workflow and Flow remain the selected names.
- Run denotes a conceptual attempt. A Task run can invoke the autonomous graph
  several times. A headless conversation also survives multiple lf invocations.
- `lf task run ISSUE` traverses the Workflow. `lf run` and
  `lf --task ISSUE run` execute work without moving it. Keep these meanings.
- Edges are named by the Flow or skill they run, unique among outgoing edges
  at a node. No new decision labels. The no-work edge to `end` keeps its meaning.
- Jack said Flow was deeply ingrained; Pipeline, Step flow and Compute flow
  were suggestions, not accepted names.

Jack's October 7 correction and stacking request establish LOO-400 / PR #1483
as the baseline. Its `e530eb780` head is integrated locally:

- `Process.lfid` is the durable identifier; optional `pid` is the observed Unix
  PID. References use `process_lfid` and `parent_process_lfid`.
- `FlowProcess`, `processes`, `flow_processes` and `flow_process_steps` replace
  the former Exec names. LOO-400 owns that migration and rename.
- Session/Run remains the interactive/headless presentation of AgentSession.
  Normal prose can say “the command exited”; Process belongs where diagnostic
  identity matters. The `lf session` API remains shared.

A Workflow owns captured definition, mutable position and move history. The
inner execution records a captured graph and step processes append-only; process
outcomes remain on Process. No primary Flow, generic Run record, stopped-Flow
resume, new driver or conversation-completion handshake is introduced.

Source decisions: LOO-386 brief and steers; LOO-400 Task and implementation;
LOO-353 history at `e67cdc62f:scratch/task-workflow.md`,
`e67cdc62f:scratch/questions.md`, and `e67cdc62f:scratch/pr-review.html`.
Historical quotations keep their original words. Old stage/TaskWorkflow/Exec
language does not instruct the new implementation. The owner-scoped command direction preserves the command meanings, node/edge
model and Process decision.

## Research: what other systems call these concepts

Official documentation inspected October 7, 2026. These are documented concepts,
not library execution tests or evidence that Loopflow users prefer a name.

| System | Documented vocabulary and behavior | Implication for Loopflow (interpretation) |
| --- | --- | --- |
| [Prefect](https://docs.prefect.io/v3/concepts/flows) | A Flow is a decorated function containing workflow logic; one invocation is a flow run. Flows can call tasks and other flows. | Flow is established and accommodates arbitrary control flow, but this usage does not teach a separate Workflow-versus-Flow distinction. |
| [Temporal](https://docs.temporal.io/workflow-definition) / [execution](https://docs.temporal.io/workflow-execution) | Workflow Definition is code; Workflow Execution is durable execution that survives failures. | Definition is a useful suffix. Temporal execution is not equivalent to one Loopflow Process; borrowing the noun does not import its durability model. |
| [Jenkins](https://www.jenkins.io/doc/book/pipeline/) | Pipelines contain stages and steps and support loops, fork/join, parallel work and human input. | Pipeline does not require a linear graph or exclude people. It is familiar software-delivery vocabulary. Loopflow must state its own review boundary. |
| [Tekton](https://tekton.dev/docs/pipelines/) | Pipeline defines an ordered collection of Tasks; PipelineRun instantiates it with execution parameters; TaskRun does likewise for a Task. | Definition and occurrence have distinct names. Loopflow should not copy Task/Run records that already have other meanings here. |
| [Haystack](https://docs.haystack.deepset.ai/docs/pipelines) | Pipelines are directed multigraphs of components; branches and loops support agentic and self-correction behavior. | Strong counterexample to ruling out Pipeline because pursue loops. Its component dataflow differs from Loopflow's command/control flow. |
| [LangGraph](https://docs.langchain.com/oss/python/langgraph/workflows-agents) / [interrupts](https://docs.langchain.com/oss/python/langgraph/interrupts) | Workflows have predetermined paths; agents choose dynamically. Graph execution can checkpoint and pause for input, continuing through a persistent thread identity. | Graph names the representation, not which of Loopflow's two graphs is intended. A human pause need not imply a distinct graph kind in other systems. |
| [Dagster](https://docs.dagster.io/guides/build/jobs/op-jobs) | An op job executes a graph of ops; one graph can supply multiple jobs with differing resources/configuration. | Job solves a binding/execution distinction Loopflow does not need to introduce. Graph is useful internally without becoming the product name. |
| [GitHub Actions](https://docs.github.com/en/actions/concepts/workflows-and-actions/workflows) | Workflow contains jobs; jobs contain steps; an event triggers a workflow run. | Workflow is also widely used for pure automation. Industry vocabulary does not reserve it for human participation. |
| [XState](https://stately.ai/docs/quick-start) / [invoke](https://stately.ai/docs/invoke) | Machine logic creates actors; a machine can invoke work and transition on completion/error. | A state machine is a useful explanation of the outer Workflow, without renaming that accepted concept or adopting actors. |
| [Airflow](https://airflow.apache.org/docs/apache-airflow/stable/core-concepts/dags.html) | Dag names the workflow model and its dependencies; the name derives from directed acyclic graph. | DAG would bring an unhelpful acyclicity expectation to Loopflow's explicit return edges. |

Observations from this sample:

1. Flow, Workflow and Pipeline overlap across products. No sampled convention
   establishes Workflow = human and Pipeline/Flow = autonomous.
2. Pipeline is compatible with loops and branches. Jenkins also directly
   contradicts an autonomous-only definition of the word.
3. Definition versus occurrence is consistently useful, but occurrence names
   and recovery semantics vary. Keep Loopflow's established Process/Run split.
4. Graph describes both Loopflow layers. It cannot distinguish them by itself.

The design implication is a local vocabulary choice, not standards compliance.
Pipeline gives the two layers more distinct names. Flow preserves familiarity
and avoids changing commands/files. Neither fixes same-name lookup on its own.

### Product vocabulary: Linear and Asana

Jack Heart called Lifecycle interesting and requested the terms used by Asana
and Linear on October 7. This expresses interest, not a naming decision.

- **Linear:** [Issue status](https://linear.app/docs/configuring-workflows)
  names the person-facing setting. The documentation calls the ordered states
  a workflow; statuses belong to fixed categories such as Unstarted, Started
  and Completed. [Issue templates](https://linear.app/docs/issue-templates)
  prefill content/properties, not an executable progression graph.
- **Asana:** workflow is the broad progression concept. Its
  [stage automation guide](https://help.asana.com/s/article/automate-how-work-moves-through-stages)
  uses sections for stages, custom fields for position and rules for automation.
  [Custom task types](https://help.asana.com/s/article/custom-task-types?language=en_US)
  define available statuses and which statuses count as complete. These types
  retain their status across projects; they are not merely display sections.
- **Lifecycle has descriptive precedent:** Asana's
  [ticket status guide](https://help.asana.com/s/article/how-to-configure-ticket-statuses)
  describes status as a ticket's position in its lifecycle. The configurable
  object on that page is Status, not a named Lifecycle definition.
- **Pipeline has a separate Linear use:**
  [release pipelines](https://linear.app/docs/releases) group releases for a
  product/environment and can have stages. This is not the issue's workflow or
  a Loopflow-like autonomous command graph.

Interpretation: these products reinforce Workflow as the conventional outer
term and Status as what people ordinarily inspect. Lifecycle remains natural
descriptive language promoted into an explicit Loopflow concept, rather than a
borrowed Asana/Linear object. Task type is another real product noun, but it
suggests classification and properties as well as behavior; using it for this
graph would require a broader decision than a vocabulary cleanup. Stage and
status name a position, while template names reusable setup; neither replaces
the whole graph. Keep the accepted node/edge representation.

The UI need not constantly display the outer type name: “feature · design” can
describe selection and position while the full noun appears in configuration
and definition inspection. This is a presentation suggestion, not an accepted
UI change or another stored status.

## Naming alternatives considered

Jack Heart found `lf pipeline` less natural than `lf flow`. Lifecycle, Progress,
Course, Track and Protocol did not resolve the naming concern. Retaining Workflow
and placing its operations under Project and Task separates command contexts
without requiring another vocabulary change. Research established no unique
industry naming rule; it did not supply measured comprehension evidence.

A Task's Workflow remains at its review node after autonomous work exits; its
conversation stays available. Failed execution is history. No naming or command
placement change introduces checkpoint resume or another driver.

## Proposed model

| Concept | Owner and representation |
| --- | --- |
| Workflow definition | Reusable nodes/edges; `WorkflowDefinition`, stored under `.lf/workflows/`. |
| Project workflow | Selection of a Workflow definition for Tasks to take up; no live execution position on the Project. |
| Task workflow | `Workflow`: captured definition, position and move history; one SQLite owner. |
| Task run | One conceptual traversal attempt; its parent Process LFID groups the child executions and Workflow moves. No new attempt table. |
| Flow definition | Autonomous steps, branches and loops; `FlowDefinition`. Composition expands before launch. |
| Flow process | The actual lf process with the captured graph and step references; `FlowProcess`. Its identity is the Process LFID. |
| Process | One actual lf invocation and its ancestry/outcome; LOO-400 owns this model. |
| Session / Run | Interactive/headless presentation of AgentSession; identity, history and feedback survive replacement of its driving process. |
| Step | An occurrence in the autonomous graph; execution references a Process and node/iteration coordinates. |

Task → Workflow → edge choice → Task-run process → one or more autonomous
processes → step processes. Independent autonomous work and associated Sessions
also belong to the Task. Latest execution grants no priority or control authority.

Use Definition for both authored layers. Do not force their live things into
identical suffixes: the outer object stores ongoing Task position; the inner process
captures one execution. Neither should be renamed Run merely for symmetry.

## Discovery and the intended interaction

Proposed owner-scoped shape; these examples are design, not installed commands:

```sh
lf project workflow show PROJECT
lf project workflow set PROJECT feature

lf task workflow show ISSUE
lf task workflow restart ISSUE

lf task run ISSUE pursue   # traverse the current Workflow edge
lf task move ISSUE demo    # set its node without executing work

lf flow list
lf flow show pursue
lf flow customize pursue
lf run pursue              # independent execution, without moving a Workflow
lf --task ISSUE run pursue # same execution, explicitly associated with the Task
```

There is no top-level `lf workflow` group. Project commands inspect/change the
selected definition. Task commands inspect/change the captured instance.
Jack Heart specified on October 7 that `restart` resets position. Implement it
as a move to `start` on the captured Workflow, retaining move and execution
history. It neither reloads the source definition nor executes work; subsequent
`lf task run` chooses the next edge. Reuse the existing move operation and its
legality rules rather than introducing another reset lifecycle. Existing
`lf task move` and `lf task run` retain their settled meanings.

Implementation choice: put reusable definition discovery and customization under
`lf project workflow list` and `lf project workflow customize NAME`. These read
or customize repository definitions; `show PROJECT` reads the Project selection
and `set PROJECT NAME` assigns it. Keep Task workflow commands about the Task's
captured instance rather than duplicating a definition catalog there.
Jack accepted `customize` copying a builtin only when needed and printing the
local path, but did not settle its placement under the new command shape. A
shared source edit must be distinguishable from changing a Task's captured graph.

Project selection supplies the Workflow at first use; an explicit Task selection
wins. Existing Tasks retain their captured graph. Current first-use selection
through `lf task run ISSUE feature` and outgoing-edge-first resolution remain
unless a separate accepted change replaces them. Coordinate Project addressing
and command ownership with LOO-397; the current CLI changes the chapter's workflow
through `lf wave update-plan --workflow`, rather than a Project command family.
LOO-397 was read October 7: it has a brief but no execution or checkout. Implement
the agreed owner paths here and keep the documentation explicit for its later
command-map pass; it is not a prerequisite for this work.

The replacement for the current Flow-inspection `--sessions` flag also needs
command-map agreement. `--processes` is a candidate consistent with LOO-400,
the implementation choice for process inspection. Keep `.lf/workflows/` and `.lf/flows/` as separate
definition namespaces; remove obsolete internal aliases at cutover.

A local autonomous definition named feature must coexist with the builtin
Workflow feature. Repository overrides apply only within the same kind.
`lf run feature` selects the autonomous definition; Task-command resolution
retains its existing policy: outgoing edge first, otherwise eligible named
Workflow, and existing ad hoc fallback when no Workflow is available. On an
unbound fixture Task, selecting feature takes up its Workflow. A Workflow-only
name supplied to `lf run` reports the Task command rather than executing it.

A malformed local definition stays unavailable with its error; it never exposes
a hidden builtin or falls through to the other kind. Store/catalog failure
preserves visibly stale last-good UI data and retry. Missing evidence stays
unknown. No Workflow and no recorded execution remain different states.

Desktop's Project workflow selector lists Workflows only. General definition
search may display both kinds, but destination, retained inspector state and
customization carry `(kind, name)`. Selecting one opens its own graph and file.
Opening another definition must preserve the Task conversation and native panes.
Use the existing explicit refresh path for catalogs and the Task part of the
watch stream for runtime data; add no polling or independent Swift authority.

## Remaining implementation

Retain Workflow and Flow throughout. Restart resets position on the captured
graph. Implement the model cleanup below with the owner-scoped commands. Resolve
routine details from existing APIs; bring consequential scope conflicts back to
Jack rather than reopening the settled names or execution meanings.

The following stays one coordinated change; do not ship a cosmetic rename that
leaves mixed lookup, wire or navigation behind.

1. **Split definition resolution and catalogs.** `engine/workflow.rs` currently
   suppresses builtins on same-named Flow files; `customize` spans both kinds.
   `flow_graph.rs::FlowCatalogEntry` has nullable graph/template/workflow payloads.
   Replace these with kind-specific loaders, `WorkflowCatalogEntry` and the
   `FlowCatalogEntry`. Keep invalid entries inspectable. General
   listing can compose them without inventing a catalog store or executable
   Workflow target. Cut Rust commands and Swift RegistryQuery, WorkModel,
   WaveWorkflowView, WorkPalette and WorkDestination over together. Delete the
   name-only `.named` preference, not just the ambiguous row labels.
2. **Align definitions and projections.** The authored `engine::flow::Flow`
   becomes `FlowDefinition`. `FlowTemplate` is a resolved disclosure
   tree, not the source definition: rename to `FlowComposition`, including wire and Swift. Keep its digest/behavior.
   Runtime `FlowSummary`, Filter, InventoryEntry, Page and Detail become matching
   `FlowProcess…` projections. TaskFlowMember/State reuse
   shared execution projections rather than implying another Task Flow identity.
   Watch `flow_runs`, Task work `flows`, `FlowRunView` and expanded-view state
   adopt FlowProcess vocabulary. Map Flow references such as `flow_id`
   and `invocation_id` to `flow_process_lfid` only where
   they identify that exact process; Workflow move `process_lfid` already names
   the Task-run process and stays distinct. Review `ops/flow_run.rs` naming too.
3. **Remove the obsolete TaskFlow bundle.** `TaskFlowSnapshot` still combines
   recommendation, latest execution and Start controls. Separate optional
   `workflow_name`, optional `latest_flow_process`, and
   `run_control` (`TaskRunControl`). The latest reading reuses process detail plus
   `TaskExecutionSnapshot` to preserve activity/refusal evidence. Delete the
   special none/latest/finished TaskFlow lifecycle; preserve every legality
   reason, roadmap indicator, completed identity and captured graph. Rust remains
   the owner and commands recheck legality. A latest row never selects a primary.
4. **Apply owner-scoped commands consistently.** Implement the agreed Project
   and Task operations and remove Workflow handling from Flow-only commands.
   Keep `.lf/workflows/`, `.lf/flows/`, Workflow, FlowProcess and LOO-400's
   process tables. Update AGENTS.md, architecture, module and Swift READMEs,
   website and builtin skills with the same model and command map. Source and
   installed packaging must agree. No internal aliases or dual writers.

LOO-400's migration stays owned by LOO-400. Projection and command alignment
alone require no second table rename. Any later persisted-format change must
preserve the released frontier and respect the parent's migration ownership;
use the repository's one-draft-per-Task operation only if required.

Preserve durable IDs, Process LFIDs/PIDs, process ancestry/outcomes, Workflow
moves, graph captures, loop coordinates, Session history, usage attribution and
native resume. Runtime DTOs/fixtures cut over in Rust and Swift together, with
required fields or explicit Optionals. Durable database/capture formats require
preservation even though obsolete internal API aliases are removed. Renaming
runtime graph types does not authorize destroying old serialized graphs. Saved
Desktop cache decoding can use its existing discard/refetch path.

## Proof and limits

Gate should exercise a provider-free path from definitions and public commands
through SQLite and the watch DTO into Swift:

1. Same-name autonomous and Workflow definitions both list, show, customize and
   open correctly. Overrides stay within kind; invalid source never falls through.
2. A Task takes up its Project Workflow. One edge's first autonomous process
   fails and its next succeeds, producing two distinct child LFIDs and one
   arrival. Independent work does not move it. Every execution remains visible.
3. Swift decodes exact shared fixtures. Header, execution log, roadmap, legal
   actions and inspectors retain Rust identities. Same-name palette navigation
   opens the selected kind without replacing Session surfaces.
4. Editing/deleting source does not alter historical captured graphs. Exercise
   missing Workflow, no execution, unknown exit, invalid definitions and failed
   reads. Fake-provider continuation keeps one AgentSession across processes.
5. Any required schema conversion preserves records from the released frontier;
   historical payloads remain readable. CLI shorthand resolution, builtin
   packaging and documentation match the final commands and owner-scoped command map.

Use existing behavioral harnesses: task_flow_launch_tests, flow_discovery_tests,
DTO fixtures and documented_commands; affected Swift DTO/TaskFlow/RegistryQuery
and navigation/view tests. Gate checks the changed-aware suite plan once,
including CLI/Rust, Desktop build and headless views, packaging/docs and website.
Rust formatting/Clippy and architecture checks apply to code changes. No tests
were run for this research and prose-only revision. Real provider continuity and
Jack's judgment of the names are not established by these planned tests.

Review findings retained: splitting catalogs alone leaves the bug if Desktop
still keys destinations by name; runtime naming must not acquire another owner;
LOO-400 supersedes the former Exec rename proposal. The research does not prove
which noun users understand best. No chapter KR or external progress is claimed.

Out of scope: retry policy, killed-process recovery, in-place graph editing, Waiting
in native terminals, completion/reopening, graph clipping, provider transport,
chapter ownership and live Home/Linear migration. LOO-353's outstanding acceptance
work remains outstanding.

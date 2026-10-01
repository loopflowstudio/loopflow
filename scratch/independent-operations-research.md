# Independent operations, optional coordination

Research and design review with Jack Heart, 2026-10-01. Current source inspected
at the growth-thoughts checkout after integration with `5eaf887fe`; installed
CLI is 0.12.28. Builds, live provider reads and proposed behavior are distinct.
Accepted direction and the workspace plan remain in
[growth-thoughts.md](growth-thoughts.md#optional-coordination-above-independent-operations--october-1-review).
[Demo evidence](task-workspace-demo.md) records the experience that prompted this review.

## Decision and limits

Jack wants small useful operations that do not require the full Loopflow workflow.
Desktop should make richer coordination readily available; CLI and agents should
invoke the same operations. Jack explicitly accepted ordinary Projects independent
of chapters, with chapters coordinating optional global realignment/reset. Jack
also accepted auditing Task admission and completion. The proposed “Task workflow”
layer is withdrawn: a Task contributes purpose, history and continuity to associated
Flows, not different execution semantics. Flow completion alone is not Task completion.

Jack requested current Project repair and implementation Tasks, then asked to do
the deeper research here. This report supplies the research now rather than queuing
an identical research assignment. The long-tail candidates below are proposals for
prioritization, not approval to implement or launch them.

## What the current code actually says

- `ops/task.rs::task_create` resolves a current Project before filing. This is a
  real dependency to evaluate, not proof that every Task needs a worktree or Flow.
- `ops/chapter.rs::select_current` requires exactly one Started Project.
  `pm::ProjectContent` requires a Flow string; `adopt_project` refuses a Started
  Project with no recorded default Flow. In this review, live Growth and Product
  each had one existing unarchived Project marked Backlog. After status repair,
  Growth's demo refresh failed specifically on the missing default Flow. These
  are two separate observations; “no current Project” did not mean “no Project.”
- `ops/task.rs::resolve_managed_task_planning` checks the Task's current Project
  and Wave against saved managed execution even with a captured Flow. Determine
  which facts protect authority and which merely select defaults before relaxing it.
- `ops/task.rs::task_complete` already has a planning-only completion path that
  allocates no execution placement. Preserve and verify it; a new universal Task
  completion subsystem would duplicate existing behavior.
- Placed completion currently checks dirty worktree state and active PR settlement,
  then invokes cleanup. Task outcome, promised delivery and disposal of a checkout
  are worth examining separately. It is not safe to simply delete the checks:
  unfinished edits and ambiguous external effects must survive.
- `docs/architecture/planning.md` already gives taskless and managed Flows one
  driver, and separates Flow completion from Task completion. `docs/architecture.md`
  already makes a single Skill executable without Wave, Project or Task.
- Unit 1 file access is already independent of PR bases and Project hydration.
  The remaining public file entry points are Task-based (`task_file`, `task_save`,
  `task_files`). A generic checkout-facing surface is a candidate, not proof that
  the underlying filesystem code needs replacement.
- Source inspection found no need for a new policy registry, controller framework,
  generic resource class or second Task workflow object.

## External evidence and useful limits

**Git: polished composition over useful operations.** Git documents plumbing as
scriptable building blocks and porcelain as the more approachable interface.
Inference for Loopflow: Desktop can make an opinionated composition the default
experience while leaving its operations independently useful. This argues for
shared operations, not for exposing raw database writes to users.
[Git documentation](https://git-scm.com/book/en/v2/Git-Internals-Plumbing-and-Porcelain).

**Linear: cadence can be optional.** Cycles are enabled per team, can automatically
create upcoming cycles and optionally add active issues, and are separate from
releases. Inference: coordination can be easy and automatic once chosen without
being required to represent work. Linear's rollover policy is not automatically
appropriate for Loopflow's chapter reset or Task cancellation rules.
[Linear cycles](https://linear.app/docs/use-cycles).

**Temporal: give a lifecycle a reason to exist.** Temporal advises against child
Workflows solely for code organization; separate execution/history can be justified
by independent resources, scheduling or scale. Inference: retain a FlowSession for
its captured execution and recovery, not a second Task-specific execution wrapper.
This is architectural evidence, not a recommendation to adopt Temporal.
[Temporal child Workflows](https://docs.temporal.io/child-workflows).

**AWS: safe composition needs explicit retry identity.** AWS describes caller request
identifiers, semantic equivalence and atomic recording of deduplication with the
operation. Inference: an ensure-current-Project operation must reconcile a lost
response rather than issuing another create. Matching title text alone cannot
prove identical intent. This reinforces the existing Ask-key direction; it does
not justify another general event ledger.
[AWS Builders' Library](https://aws.amazon.com/builders-library/making-retries-safe-with-idempotent-APIs/).

**Kubernetes: an operation and its reconciler are different owners.** Controllers
observe resources, act toward desired state and report the result. Multiple
controllers distinguish the objects they own. Inference: ensuring a primary
conversation or current Project can be optional orchestration over ordinary
objects, with explicit ownership and stop behavior. Do not import a cluster or
permanent service merely to implement this relationship.
[Kubernetes controllers](https://kubernetes.io/docs/concepts/architecture/controller/).

**LangGraph: a saved pause does not make effects exactly once.** Its interrupt
documentation says nodes restart on resume and recommends idempotent preceding
effects or separating them. Inference: optional review coordination still needs
exact boundary identity and safe retry. Removing coupling must preserve saved
answers, claimed execution and uncertain external effects.
[LangGraph interrupts](https://docs.langchain.com/oss/python/langgraph/interrupts).

## The design test

For each operation, name the object changed, the required facts, the authority,
and the retry identity. Ask whether those inputs still make sense when no higher
workflow is selected. Then test adding, changing and removing that coordination
without replacing the object's identity or erasing evidence.

Intrinsic guarantees stay below: revision checks, legal state transitions,
transaction boundaries, exact driver ownership and truthful completion. Defaults,
cadence, discovery and multi-object sequencing live above. Missing optional context
must not block valid operations; missing required authority still must.

Examples, proposed behavior rather than implemented command syntax:

1. Open a Wave. Its explicit activation path finds its current Project or ensures
   one. Ordinary list/status remains read-only. A repeated activation returns the
   same Project; a provider timeout preserves uncertainty and retries the same intent.
2. File a research Task and complete it with a result. No checkout or PR is invented.
   Later attach an implementation Flow to another Task; Flow replacement preserves
   the Task and prior work. Existing planning-only completion is the starting point.
3. Preview global realignment. Apply selected Project resets with durable per-reset
   receipts. A halfway failure leaves unaffected Projects, Sessions and files usable.
   Retry resumes the reset; it does not silently cancel untouched work as recovery.

## Ranked candidate backlog

Rank is a recommendation based on observed friction and reuse. Relative size names
risk/scope, not a schedule estimate. Every row needs design review before launch.

| Rank | Experienced improvement | Smaller owner / optional composition | Evidence and next proof | Relative scope |
| --- | --- | --- | --- | --- |
| 1 | Opening a Wave always finds somewhere to put work | Ordinary Project / ensure-current and optional reset | Live missing-current/default-Flow failures; retry, concurrent discovery and ambiguous ownership proof | Medium |
| 2 | Start and finish the kind of Task intended | Task outcome / selected execution and delivery promises | Admission/completion source findings; planning-only and managed delivery scenarios | Large |
| 3 | Keep working when unrelated planning is unavailable | Per-operation evidence / portfolio aggregation | Demo-wide failures and scoped destination repair; break one Wave and use another Task | Medium |
| 4 | Use a Project created outside Loopflow without rewriting it into a template | Provider Project facts / Loopflow defaults and optional chapter participation | Required content/Flow conversion; adopt arbitrary name and preserve freeform content | Medium; overlaps rank 1 |
| 5 | Continue a captured Flow after harmless planning reorganization | Flow execution authority / Task association and current defaults | Managed planning match in `resolve_managed_task_planning`; move Project then resume exact saved boundary without weakening ownership | Large |
| 6 | Record Task outcome without losing a useful checkout | Outcome and delivery evidence / cleanup | Placed completion calls cleanup; retain dirty research artifacts and distinguish complete, delivered and disposed | Large; design with rank 2 |
| 7 | Open any checkout's files without inventing a Task | Checkout/revisioned document / Task grouping and PR comparison | Task-based public entry points, reusable no-PR reader; identical edit/conflict/symlink behavior outside a Task | Medium |
| 8 | Add work to a Task without relaunching it | Stable Session/Flow identity / attribution | Existing additive Task association; attach existing conversation and retain provider identity/history | Medium; extend LOO-358 rather than duplicate |
| 9 | Reliable primary conversations without controlling ordinary conversation lifetime | Session / scope binding and explicit ensure | Existing LOO-364 direction; two openers converge, explicit reset affects one scope, deliberate stop does not cause unwanted resurrection | Existing Task |
| 10 | Change defaults without altering already-started work | Captured inputs / scope defaults | Existing pinned Flow behavior; new selection uses new default while prior execution resumes unchanged | Small/medium |
| 11 | Recover one failed coordination step without rebuilding everything | Durable operation receipt / batch coordinator | Interrupted chapter transition; resume exact remaining operations while untouched work remains usable | Medium; part of rank 1 if needed for reset |
| 12 | Understand why an action is unavailable at the point of action | Existing legal-action reading / Desktop explanation | Current action evidence owners; distinguish optional setup from missing authority, offer only relevant next action | Small/medium |

Ship the first two through their scoped Tasks. Treat ranks 3–12 as a long tail to
compare against existing work, not ten new issues. Several are extensions to
LOO-353, LOO-358 or LOO-364, or test obligations for the first two. Prefer deleting
an unnecessary dependency over introducing a new product concept.

## Counterexamples the design must survive

- A Flow result cannot complete a Task with an unresolved promised delivery.
- An unavailable provider response cannot create a second current Project.
- A higher-level reset cannot erase a lower-level draft, checkout or saved answer.
- A Task reassignment cannot silently acquire another execution driver's authority.
- A title/slug match cannot replace stable identity or prove retry equivalence.
- A read-only CLI invocation cannot become an unannounced provider mutation.
- Removing Desktop cannot remove the only implementation of correctness or recovery.

## Filed work and immediate repair

- [LOO-366 — Keep every Wave ready for work without requiring a chapter](https://linear.app/loopflow/issue/LOO-366): current Project ensure/reuse, independent Project content and optional coordinated resets.
- [LOO-367 — Start and finish Tasks without unrelated workflow prerequisites](https://linear.app/loopflow/issue/LOO-367): admission/completion audit, preserving existing planning-only completion and actual delivery promises.

Both are filed under Infrastructure; neither was launched. No duplicate research
Task was filed because Jack requested the research in this conversation.

Fresh provider reads found exactly one unarchived Project each for Growth and
Product, both Backlog. Authorized status updates returned success and In Progress
for the same IDs. Existing plans were preserved and their `feature` fallback
recorded explicitly. This repairs current data; it does not implement optional
chapters. The source reader then exposed foreign-Team migration records being
adopted before the ordinary ownership filter. That local repair preserves those
Projects and receipts. Configured verification belongs in the
[demo record](task-workspace-demo.md).

Review conclusion: the smaller API must own safe retries and authority. Optional
coordination cannot mean optional correctness. Existing planning-only Task
completion and the shared Flow driver are counterexamples to inventing new layers.
The first two briefs target concrete dependency removal; the remaining proposals
need evidence and overlap review before becoming Tasks.

## Next review

Choose the concrete lower operations and their contracts for Project availability
and Task admission/completion. Verify existing behavior before treating a proposed
feature as missing. The research supports a direction and ranked candidates;
it does not establish that every listed dependency should be removed, nor that
another workflow framework would make Loopflow smaller.

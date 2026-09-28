> Execution context: LOO-332. Primary design: `scratch/task-automation.md`.
> Source: `/Users/jack/src/loopflow.discord/scratch/recursive-vsm.md` at `613a66ca8fd99b81b92a902c986dfc1540d6ca10`.
> Stacked on LOO-298 at `d07e569330c8dedceb5dd238ce9b22a7b6137006`. This destination owns ongoing edits.
> Inherited LOO-298 scratch is dependency evidence, not this Task’s assignment.

# Recursive Wave operation and repository VSM

2026-09-28 · Jack Heart · Product · Independent launch-plan outcome

## What to build

Give every Wave S1–S5 thinking within wave-operate and add one finite
repository-wide vsm-operate Flow that applies the same functions across Waves.
Both must be useful as manual invocations before scheduler/Discord integration.

Jack: "so vsm is meant to be recursive" and "So yeah, i guess Wave gets s1-s5".
The repository operation is "just one for the whole repo". Identity must be
"something being determined through the whole graph".

Projects are part of their Waves; no Project operator. task-operate selects and
shepherds the intended Task Flow through landing; do not turn it into another
mandatory S1–S5 reporting cycle. Neither Wave nor repository operation is an
execution prerequisite for Tasks. No resident, new supervision hierarchy, or
five scheduled agents per Wave.

## Current system

`engine/builtins/wave/skill/wave_operate.md` already implements read/decide/act
with one or two useful moves. Its old final-answer/chat and runtime references
need reconciliation with LOO-298's daemon deletion. Existing `goal/s1.md`
through `s5.md`, `s2-scan`/`s2-assess` through `s5-scan`/`s5-assess` and
`flow/govern-*.yaml` contain the doctrine, currently at chord/member-Wave scope.
Reuse the meaning without copying obsolete resident or chord configuration
requirements into the new flow. Builtin skills must remain self-contained.

## Behavior

Within a Wave:

- S1: are Tasks delivering useful outcomes, independently?
- S2: are their interactions and dependencies working without repeated conflict?
- S3: is current capacity advancing the Wave's outcomes rather than busywork?
- S4: what changed in user needs, the code or environment that calls its plan
  into question?
- S5: do purpose, boundaries and the current plan still describe useful work?

At repository scope, consider the same questions across Waves. A Task finding
can challenge a Wave premise, and a Wave finding can challenge repository
direction. Direction and resolved decisions inform work back down. Preserve
evidence and disagreements; do not silently replace another owner's objective.
Use existing authorized Task/planning/memory operations for action. Changing
fundamental user direction or exceeding authority remains a review question.

Each pass reads dated outcomes and unresolved concerns, distinguishes evidence
from hypotheses, selects one or two consequential moves, and exits. No five
reports every tick. A quiet channel does not establish health. Reading breadth
should follow the actual question: summaries and relevant evidence first,
not all raw transcripts. Keep unresolved concerns in their existing owners.

This Task owns the prompt/Flow definitions, invocation contract, docs and proof
of a useful finite pass. It does not own cron installation, Task admission,
landing, database schema or Discord transport. Operate with no chat connection;
do not invent commands for future chat APIs. Name chat integration as a follow-up
in the docs only where it helps users understand current behavior.

## Flow and invocation contract

Reuse the existing wave-operate entry point and add an explicitly invokable
`vsm-operate` Flow through the builtin catalog. Inspect actual Flow composition
and validation before choosing its graph; the smallest graph that reads,
judges and acts once is preferred. No persistent governance cursor is required.
Keep VSM doctrine shared where the existing authoring system supports reuse;
avoid a new plugin or orchestration framework for text reuse.

Existing Wave/Task values and evidence remain the data model. The only new
public surface is the finite repository Flow; scope resolution must be explicit
and must not infer an unrelated Wave. Repository operation can read the roster
through ordinary `lf` queries. Offline provider evidence stays stale/unknown;
it must not become permission to close work or invent progress.

## Demo and done when

Invoke wave-operate for a selected Wave; show a useful judgment/action touching
the appropriate S1–S5 concern without launching five agents. Invoke vsm-operate
for the repository; show a cross-Wave tension evaluated with source evidence.
Demonstrate a bottom-up finding challenging higher-level identity and the
resulting decision returning to the relevant Wave, without asserting that the
repository operator alone defines identity.

Validate builtin references/catalog expansion and existing skill alignment.
Review scenario outputs for ordinary progress, a coordination conflict, new
external evidence, identity disagreement, stale evidence, and no useful action.
Label model simulations as such. A real authorized finite pass is stronger
evidence; never mutate unrelated live Tasks just to manufacture a demo.
Task progress must remain possible when both operators are absent.

## Execution

Stack on LOO-298 so documentation and builtin references target its deleted
daemon surfaces. Changes are primarily within builtin Wave skills/flows and
their user docs; do not take over LOO-298 or the Task automation core.
Use the supplied design as the starting point. Accepted scope is above; exact
Flow graph is an implementation choice to explain during review. The configured
execution Flow must retain human demo/review. Record actual checks and remaining
live evidence here; no implementation has been performed by launch-plan.

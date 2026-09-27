# Workspace UX design review

2026-09-26 · LOO-303 · Approved interactive design review

## Design and evidence

- [Current design](workspace-ux-on-data-model.md)
- [Current assumptions and inherited agenda](questions.md)
- [Recorded product decisions](from-loo291/demo-native-workspace.md)
- [Session ownership correction](session-runs-and-current-run.md)

The current participant (name unresolved) approved the corrected design:
“ok, approve the design then i guess”. Approval covers the design and recorded
scope corrections, not implementation, installed acceptance or a Flow navigation
decision.

## Flow placement feedback

The current participant (name unresolved) said: “I dont think taskless lfows need
to show up in the main pane. they will be included in our overall monitoring data
but thats about it”, then “Well i think waves will needa  flow UI”.

The participant then clarified: “but yeah i dont think we need something like
the task page for flow invocations outside task”. Finally, quoting “The Project’s
default Flow for Tasks remains a separate setting”, the participant said:
“This is waht i meant by wave flows”.

The final direction replaces the agent's mistaken Wave-execution interpretation:

- Wave page: the current Project's default Flow template for its Tasks, folded
  and disclosable using the shared template UI. Project retains ownership.
- Task page: its own captured invocation and attempt detail; explicit Flow
  overrides remain supported.
- Non-Task invocations, including Wave-attributed execution: overall monitoring,
  with no standalone main-pane inspector or Task-like page.

Removed the invented Wave-level execution surface and its choose/start question.
No new Wave Flow owner or Wave execution controls are required. Session inventory
and orphan membership remain unchanged as an implementation assumption; the
comments concern invocation presentation and do not expressly remove conversations.

## Opening findings

Read-only source inspection confirms the kickoff's main constraints: the Flow
loader flattens composition, the current Session command has no bind operation,
and the public Task Flow projection lacks ordered attempts/current attempt.
The running line selects a provider by Task and elapsed time from Task update
time. The new UI needs the exact Run projection before it can label attempts.
The existing terminal host stays mounted while hidden; a room must transfer
the sole mount without releasing the retained terminal.

Palette, Task links and folded templates can proceed independently of the
remaining shared Session conversion. Room/bind and attempt presentation consume
that conversion; this Task does not implement a second storage path.

## Retained implementation assumptions

The overall design is approved. These defaults remain explicit rather than
being represented as separately discussed product decisions:

1. Task links and palette Task entries open Task details even with one Session;
   existing sidebar drill-down remains unchanged.
2. Independent Task Runs mark the Task Started but leave its folded Flow template
   visible until an invocation exists. Inspection never starts execution.
3. Bind assigns the Session's current Run; previous Runs retain attribution.
   The inherited nullable invocation/Task equality also means a taskless Flow's
   Run cannot independently bind to a Task. Preserve the parent contract and
   shared explanation; this review does not authorize changing that invariant.

The existing scope remains light only, preserving the installed composition,
with every orphan tiled and one permanent-target confirmation for bind.

## Next useful action

Mark the review ready with this note, the current design and questions paths.
The user completes the Session; the following loop-decide owns navigation.
The next implementation cut is shared destination routing for ⌘K and Task
links, followed by folded templates including the Project default on the Wave
page. Room/bind and attempt UI consume the completed parent contracts.

No blocking design question remains for the independent slices. Keep the parent
integration checklist and all configured acceptance obligations; design approval
does not claim those are satisfied. Do not complete this Session or select a
navigation edge from this review.

## Verification

Working-diff whitespace passes. The review changes scratch Markdown only;
product tests are unnecessary. Source inspection supports the findings above,
but no new behavioral, native or installed acceptance proof was collected.

No implementation, product tests, installed changes or external Task/PR writes
were performed during preparation.

# Data model review feedback

2026-09-26 · LOO-298 · Interactive review with Jack

Status: approved to proceed with the recorded changes; documentation verified
and ready for handoff. No navigation decision is recorded.
The opening briefing addressed Jack from the supplied context. A subsequent
participant update states that the current participant's name is unknown;
the feedback below is not attributed to Jack.

## Design and evidence

- [Current design](data-model-one-table-per.md)
- [Review agenda](questions.md)
- [Jack's recorded decisions](from-loo291/demo-native-workspace.md)
- [Target model reference](../docs/architecture-reference.md#core-models-and-apis)

The branch was clean when this review began. Source inspection confirms that
`ops/human_session.rs::list` combines four Session sources and that
`ops/flow_run.rs::create` persists a Flow with an optional Task. The latter is
an existing execution path, not merely an old documentation example.

## Agreed design changes

The current participant (name unresolved) explicitly stated: “Flows should be
allowed to haven o task, why would htey not”.

The draft incorrectly converted Task ownership of some invocations into a
requirement for all invocations. Invocation Task is now nullable. Taskless and
Task-owned Flows share the same SQLite records, captured execution and review
recovery. Migration preserves existing taskless invocations without assigning
synthetic Tasks or forcing completion. The design, architecture docs, CLI
examples and contributor guidance now reflect that correction.

Task still implies Wave. The existing equality between a Run's nullable Task
and its invocation's nullable Task is retained pending the binding discussion;
this correction alone does not authorize changing execution membership.

The participant then confirmed “ok so session not a child of run after, session
has runs”, added “and current run”, and concluded “I like flow and invocation
fine” and “ok approved to proceed with those changes”.

Session now owns Runs and has a current Run. Session has its own stable ID;
Run has a nullable Session FK. Current Run must belong to that Session.
Replacement adds a Run and atomically switches the pointer and pending attempt,
retaining Session title/feedback and all older Run evidence. There is no name
copy. Flow remains the template; Invocation names its execution.

The [Session ownership note](session-runs-and-current-run.md) records the
concrete model, preservation obligations and the unconfirmed bind assumption.

## Unresolved questions

Taskless execution is settled. Binding an individual Run of a taskless Flow
to a Task needs review against the retained nullable parent equality; do not
claim that the participant approved that restriction.

Historical unknown membership and writable-store requirements retain their
existing design treatment; neither received separate explicit confirmation.
Cross-Run bind scope retains current Run attribution as an implementation
assumption. Approval of stable Session identity does not imply approval to
rewrite every historical Run's Task or change invocation membership.

## Next useful action

Use the amended design for subsequent work. Preserve the complete migration,
execution and deletion proofs; the corrections do not reduce those obligations.
Mark this review ready with these note paths. The participant completes the
review; the following loop-decide owns navigation. Do not complete the Session
or record Advance/Iterate from this review.

## Verification

Documentation-only correction. Whitespace checks, portable HTML consistency,
and both focused website documentation tests pass. The initial HTML check
failed because rendering preceded synchronization of the website's Markdown
copy; regenerated after synchronization and repeated the affected checks.
No runtime implementation or real-Home migration was performed.

After the Session ownership revision, portable HTML was regenerated from the
synchronized Markdown; its consistency check, whitespace check and both focused
website tests passed again (2 passed). A targeted search found no remaining
one-Session-per-Run claims in the edited specification and contributor docs.
The architecture inventory's previously recorded source/spec gaps remain; this
review does not claim an implementation gate pass.

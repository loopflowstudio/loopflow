# Data model review feedback

2026-09-26 · LOO-298 · Interactive review with Jack

Status: briefing prepared; no new confirmation or design change recorded yet.

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

None in this review yet. Jack's previously recorded decisions remain the basis
of the design; this briefing does not reconfirm the elaborated mechanisms.

## Unresolved questions

First discussion: does requiring a Task for every Flow invocation match Jack's
intent? The draft removes taskless Flow execution while retaining taskless
Skills and prompts. Preserving taskless Flows would require explicitly revising
the invocation ancestry contract, not creating synthetic Tasks or maintaining
a separate file-backed execution path.

The remaining agenda covers historical unknown membership, Session identity
when a published Run is replaced, and the operational consequences of the new
owners. Naming retains the approved Flow / Flow invocation default.

## Next useful action

Jack reviews the briefing and settles taskless Flow behavior. Record feedback
and update the design as decisions land, then reconcile the remaining agenda.
Do not mark the review ready or claim confirmation before that conversation.
This review records no Advance or Iterate decision.

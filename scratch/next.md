# Slice 6C: delete producerless evidence Receipts

## Implement

- Delete the evidence `Receipt` model, kinds, parser, and `lf receipt` resolver
  command. No current feature authors these pointers after memory became a file.
- Delete receipt-only resolved-record DTOs, docs, tests, and authored-flow
  dispatch.
- Delete receipt-only PR reference/identity helpers and the all-Task-PR store
  query. Keep the CI repository identity path directly on current Task PR data.
- Retain generic mutation result types named `*Receipt`; those are operation
  outcomes, not evidence pointers.
- Add no tombstone command or compatibility alias.

## Done when

- [ ] the evidence Receipt module and CLI command are absent.
- [ ] no current producer, parser, resolver, DTO, or docs remain.
- [ ] PR/CI behavior uses direct Task PR fields without receipt identity types.
- [ ] authored flows reject `receipt` as a command and focused behavior, fmt,
      and all-target clippy pass.

# Final integration notes

- `handback_state` is now restricted to live `tui` and `ide` AgentInvocations
  with no `answer_ask_id`. Core and answer invocations cannot be ended through
  the human handback command. Only `succeeded` advances Demo.
- After runner loss, `lf ask wait <ask-id>` may recover an exchange from an
  earlier AgentInvocation in the same Work Epoch. The explicit id is the fence
  that distinguishes recovery from a new Turn's own Ask.
- Linear's existing comment mutation targets issue ids, so the durable outbox
  mirrors Task Ask/Answer exchanges. Project and Wave Work need a separate
  Linear Project-update target before their exchanges can use the same path.

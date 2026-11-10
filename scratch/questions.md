# LOO-802 assumptions

- The brief's “Additive to revision 2” predates this checkout's revision 4.
  Document events alongside the current protocol without reverting its revision.
- With no earlier design in scratch, use `event {id,seq,event}` for the envelope:
  subscription ID, connection-local sequence, typed payload. This is a routine
  wire naming choice for implementation to reconcile, not a new product decision.

- Interpret “Task shipped” as the existing recorded PR merge, not Auto-Ship
  approval or Task completion. Step events report recorded exits, including
  failed/interrupted outcomes; no process-liveness guesses.
- A sequence gap ends Swift event-consuming streams with a resubscribe error;
  unknown kinds end only the affected event consumer. Reading-only watches stay
  connected. Reopening starts from a complete reading.

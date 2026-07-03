# Queue

Approved reduction work decomposed for workers lives here.

Statuses:

- `ready` - worker can take it.
- `in-flight` - worker owns it now.
- `done` - shipped and verified.
- `blocked` - cannot proceed without a recorded blocker.

Queue items should be small enough for a worker to finish and verify, but the
proposal that produced them may be large. Reduce gates on design agreement, not
diff size.

# LOO-775 assumptions

2026-11-08: resume compares current state, not every intermediate change. Tokens
are opaque fingerprints tied to one server incarnation, avoiding retention
configuration or payload caching. Matching arguments/selection are required;
a new stream starts fresh. Desktop retries connection loss within its retained
stream; a failed reconnect surfaces its error through the existing window retry
path, starting fresh, rather than hiding a prolonged outage. Refusal and buffer
overflow also end the stream.

No unresolved product choice.

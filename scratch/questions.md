# LOO-775 assumptions

2026-11-08: resume compares current state, not every intermediate change. Tokens
are opaque fingerprints tied to one server incarnation, avoiding retention
configuration or payload caching. Matching arguments/selection are required;
a new stream starts fresh. Desktop retries connection loss within its retained
stream; a failed reconnect surfaces its error through the existing window retry
path, starting fresh, rather than hiding a prolonged outage. Refusal and buffer
overflow also end the stream.

`lf context`: Product memory exceeds its token target by 417; Wave context is
read-only in this Graph step under the work guidance, so budget curation remains
with Wave maintenance. Proposal: replace “Work and continuity”'s retired Run,
Ask, chat and provider-topology recipe with a reference to
`95c17782a2dfd782a707f2a9beac6fe8cbdf9be3:.lf/wave/product/MEMORY.md`, that heading;
keep the environment/identity distinction. Also deduplicate “Optional Task
membership”'s repeated native-proof caveats and “Task delivery boundary”'s
code-recoverable lifecycle description. Preserve decisions and run results.

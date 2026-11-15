# Desktop reactivity discussion

Jack Heart:
- “another thing i will add is that completing a task should make it disappear immediately”
- “can we talk about what are the fundamental causes of so many intermediate states and non-immediate reactivity?”
- “is it about slow sql? insufficient reactive UI archtiecture?”

Jack Heart, subsequent scope direction:

“Let's make sure this is an aggressive systematic rewrite that sets us up to avoid these kinds of issues across the whole system, not just these specific interactions I'm talking about, and mobile and desktop are being considered”

This supersedes the launch-only scope. Reassess the shared interaction model and
cross-platform migration before continuing isolated screen fixes. Launch and
completion are representative journeys. The design should cover immediate action
response, pending operations, authoritative reconciliation, subscription freshness
and presentation ownership across Desktop and mobile, removing competing older
mechanisms. Bring the design and migration scope to this conversation for review.
No SQL timing trace has been collected in this conversation. Shipping is not
approved by this direction.

Jack Heart, architectural option:

“That could include having a whole second store or a list of pending changes that are applied as a diff to the known store, something like that”

Evaluate a client store and a pending-operation overlay over authoritative
snapshots. Neither mechanism has been selected in this conversation.

Jack Heart, cached navigation latency:

“another thing that is too slow is the time it takes for a task to look highlighted (and the session to render) after i click it, evne if its cached / recetn”

Include Task switching alongside launch and completion. Measure click-to-highlight,
cached-content presentation and input readiness separately. Selection changes
before I/O; usable cached content displays while refreshing. Latest selection wins
under rapid switching and reordered responses. Investigate retained native surfaces
and main-thread work as well as cache/transport latency.

Jack Heart:

“or for a wave to expand when i click on it”

Wave disclosure must react before loading children. Include local navigation and
disclosure state in the responsiveness audit, alongside synchronized mutations.

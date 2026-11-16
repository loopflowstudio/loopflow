# Desktop reactivity discussion

Jack Heart:
- “another thing i will add is that completing a task should make it disappear immediately”
- “can we talk about what are the fundamental causes of so many intermediate states and non-immediate reactivity?”
- “is it about slow sql? insufficient reactive UI archtiecture?”

Jack Heart, subsequent scope direction:

“Let's make sure this is an aggressive systematic rewrite that sets us up to avoid these kinds of issues across the whole system, not just these specific interactions I'm talking about, and mobile and desktop are being considered”

This supersedes the launch-only scope; launch and completion are representative
journeys. Shipping is not approved by this direction.

Jack Heart, architectural option:

“That could include having a whole second store or a list of pending changes that are applied as a diff to the known store, something like that”


Jack Heart, cached navigation latency:

“another thing that is too slow is the time it takes for a task to look highlighted (and the session to render) after i click it, evne if its cached / recetn”


Jack Heart:

“or for a wave to expand when i click on it”


Jack Heart, 2026-11-14, comment `25b9206c-8fa8-4be9-aa7e-a69c3daa8eee`:
> “Have we learned anything about what’s going on? Also should we add some benchmarking?”

Host direction, forwarded in comment `7a62299f-7430-4c8a-89ad-a398766ca0a7`:
> “I think doing it on my machine under load is probably somewhat problematic. We can probably do it on the mini as long as the mini isn’t loaded and I think that’s fine basically”
> “You can also use this machine if the load is not too high”
> “Or even theoretically, you could rent a Mac from AWS if you want”

Jack Heart, comment `35145bd7-dbd3-40db-9081-d75844fb0f40`:
> “Agreed. Yeah prefer the quiet mini if that’s an option”

Benchmarking now belongs to implementation as well as demo. ADEMark coverage and
workflow-specific journeys are mapped in the existing performance harness docs.

Jack Heart, 2026-11-15, conversation discussion:
> “I wonder if we should change the skills that we give in Implement and, to maybe some lesser degree, Kick Off and Design (or maybe not even lesser), to encourage slices that are big enough to get full vertical views on a system and allow you to see it working end to end, even if it's not in every case, rather than trying to design a perfect version of a layer and then going to the next layer only to figure out that actually your previous implementation was not correct.”
> “second there's the question of how we are going to review this thing. Can we decompose it into smaller shippable chunks?”
> “And third there's the question of what the product questions that arose were and what we should discuss in order to answer them or guide the overall design”

These are discussion questions, not a decision to split, ship, change product
behavior or restart the active Graph.

Jack Heart, 2026-11-15, forwarded in comment `90a2c53d-50d4-4962-b4c7-65af61e1943d`:
> “so that we can use the benchmark to prove each subsequent step”

Benchmark-first extraction is handled in the primary conversation. Finish and
checkpoint the current Start/Ship/Hold cut, then stop rewrite iteration at the
review/continuation boundary until that independent benchmark is available.

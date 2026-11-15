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

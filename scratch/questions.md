# LOO-816 assumptions

The Task brief requires standard/default and fast/priority values but names no
preference key or canonical spelling. The implementation uses
`codex.service_tier`, canonical `default`/`priority`, and `standard`/`fast`
aliases. These are reversible implementation choices, not quoted decisions or
open product questions; they fit the requested values and existing catalog.

Replay retains the captured tier, following its existing recorded-input contract;
Session resume that launches Codex again resolves the current preference, like
other launch settings. Reattachment keeps the running conversation's tier.
The concept review found no additional product choice here. An absent captured
tier remains absent on replay; it does not snapshot Codex's effective default.

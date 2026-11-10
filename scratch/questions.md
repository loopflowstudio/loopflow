# LOO-816 assumptions

The Task brief requires standard/default and fast/priority values but names no
preference key or canonical spelling. The implementation uses
`codex.service_tier`, canonical `default`/`priority`, and `standard`/`fast`
aliases. These remain reversible choices for demo, not quoted decisions.

Replay retains the captured tier, following its existing recorded-input contract;
ordinary Session resume resolves the current preference, like other launch settings.

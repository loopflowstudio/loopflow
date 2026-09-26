# Task entry experiment — 2026-09-25

Subsequent human clarification changes sidebar membership to started Tasks only.
The prototype now uses an explicit sample `started` fact, independently of
Sessions and status. Wave pages retain the full plan; inspecting an unstarted
Task does not add it. Starting its sample conversation does. A focused browser
check passes those transitions and simulates Session removal to verify a started
Task remains visible without a count. See `wave-working-set.json` and the
visually inspected `started-tasks.png`. Earlier source hashes/captures describe
the preceding Task-entry version; they are not current-source receipts.

A uses one Task row with an inline open-conversation count. Clicking the name
opens its one local Session with context; clicking the count opens the overview.
Zero/multiple conversations open the overview; selecting a conversation from
the picker foregrounds that exact Session. External conversations remain explicit
opening choices. B/C retain their prior nested rows.

Opus authored the bounded prototype change through `lf -m claude`. Parent review
found a reachable failure: the old laptop media rule hid the Conversations list,
so after removal of nested rows, multiple conversations could not be selected.
The browser check timed out trying to click the existing but invisible Session
button. Removed that hiding rule; the two columns remain side by side at laptop
width and stack below 860px. No other source correction was needed.

The initial zero-conversation assertion incorrectly expected exactly one Start
control; existing overview and pane each offer it. The corrected assertion
requires an available action and no invented draft/Session. This was a test
assumption correction, separate from the real hidden-picker defect.

Browser proof passes at 1400×780 and 1100×720: initial Overview, no nested Session
rows or Task disclosure, count 1 including an external Session, no badge for zero,
direct single-Session entry, count-based inspection, disabled implicit external
transfer, two conversations created through the sample UI, exact picker selection,
Unicode draft retention, shell/Monitor restoration, and repository round trip.
Both sizes have no JavaScript errors. `interactions.json` records the pass.
Overview captures use `lf screenshot`; both were visually inspected, including
the corrected laptop layout. Long Task names truncate at these widths.

This tests sample JavaScript state, not native surfaces or configured providers.
The one-Session click currently hides the sidebar because it uses the existing
Session-with-context depth. That interaction and the count-click/zero/multiple
fallbacks remain proposals for human feedback. No native code, installation,
PM record, provider, commit or publication changed.

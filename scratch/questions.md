# Working assumptions

- Jack Heart replied “1a” to the 2026-11-15 conversation review's disconnected-edit
  question: save and project Complete/rename/move locally, show “Waiting for
  connection,” and deliver on reconnect to the original Machine. Retain across
  restart; recover uncertain sends without duplicate execution. Implementation
  remains required on both clients.

- Scope now covers Desktop and mobile interaction state throughout the app;
  launch, completion, cached/recent Task selection and Wave disclosure are
  representative acceptance journeys. The earlier
  Desktop-only boundary is superseded by comments b38d7ddf/e008238d/30bf26ae.
- Headless engineering choice: use authoritative cache plus ordered operation
  overlays and transactional outbox for the next implementation; no requirement
  calls for a full mutable offline replica. This is not a recorded product
  preference. Validate membership coverage during implementation. The outbox uses system
  SQLite through SQLite3; this reversible choice avoids another dependency.
  GRDB is not adopted. Protocol revision 6 now adds stamped subscriptions;
  confirmed Work bases and ordered completion effects now share the outbox.
  Shared synchronous Complete now consumes those effects on both hosts; provenance
  and full membership migration remain open.
- Completion navigation: Jack Heart replied “1b for the question (or show noting)”
  to the numbered choice in `scratch/concept-review.html`. Remove the row and
  select the next Task immediately; an empty state is allowed when none remains.
  This supersedes the draft keep-conversation-open assumption. Failure or refresh
  never steals a later deliberate selection.
- Proposed 100 ms P95 local reaction target, at least 20 native samples per
  journey/platform. Native reaction, input readiness and full backend stage timings remain
  unmeasured; no SQL diagnosis is asserted. Implementation now owns benchmark
  collection too. Both hosts failed load preflight; the mini also runs macOS 15.5
  against Desktop's macOS 26 target. Current debug 10k constructor samples are
  diagnostic only; a valid native baseline remains open.

- Admission identity: headless engineering choice is immutable submitted content
  plus deferred canonical creation identity. Released rows bind the first verified
  envelope through recoverable candidate validation in the same receipt owner;
  candidate conflict never replaces the existing creation outcome. This is an
  engineering choice, not a recorded product preference.

- Released creation journal files have no recorded send epoch. Import retains
  their exact command, but a missing receipt cannot distinguish an unsent file
  from an effect lost by historical restoration. Keep it inspectable instead of
  replaying it. A present legacy receipt can adopt the saved envelope.

- Stage resolutions stay in the existing semantic `commits` entries, with a required
  typed disposition. Supersession results identify the withdrawn request; clients
  need a covering replacement reading before showing the unoverlaid membership.
  This avoids a second resolution ledger and guessed replacement Task values.

- Other command families retain conservative per-Machine reload requirements
  while their declaration and projection policies remain incomplete. Full
  cross-platform interaction cutover is not claimed.

- Completion now transfers lagging stored-query effects on retirement; query
  open/close leases belong to the presentation owner. Closed bases now use compact
  scoped reload requirements instead of per-operation effects.
  New completion reload requirements are scoped to repository and declared effect;
  preexisting broad requirements retain their conservative meaning until epoch reset.

- Query destinations retain the requested expression when canonical resolution is
  pending. A saved query opened through an expression retains that identity in
  history; a saved Dashboard selected by ID keeps its ID. Deferred Machine
  discovery binds an unsaved Complete through the original store endpoint before
  saving; it never retargets a saved command. Prepared query entries use
  the shared owner's bounded reusable set across close/reopen. Headless reversible
  sizing: eight unleased prepared answers plus pinned live queries, with at most
  64 retired effects per retained base; exceeding that budget makes the base cold
  under its reload floor. These are internal cache sizes, not product preferences.

- Creation keeps retired imported/editor-linked identities in the same SQLite
  owner to prevent source-file resurrection after a crash. Indexed pending reads
  exclude them. Editor revision IDs distinguish a newer unsent draft. Legacy
  files have no submission time; their modification date seeds the retained record.
  These are internal migration choices, not product preferences.

- Creation action labels, headless reversible choice from the 2026-11-15
  concept-review: one vocabulary on both hosts, replacing Desktop "Retry
  recovery" and phone "Check status" for the same call. Proposed: **Try again**
  (never saved), **Edit request**, **Check request** (saved, outcome unknown),
  **Retry opening** (Session recorded, not attached). Copy only; nobody has
  stated a preference. Request rows currently sit above the Task list on both
  hosts; their position has not been discussed.

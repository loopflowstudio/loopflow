# Working assumptions

- Implementation, 2026-11-15: dependent field/manifest/Move work stopped on a
  concrete clock counterexample. OS process exit changes activity and ordering
  with unchanged SQLite revisions. The proposed per-field domain fences cannot
  order those inputs; exporting `observe` counters without an incarnation also
  reuses revisions after eviction. Proposed correction and acceptance are in
  `instant-task-session.md` (External-observation counterexample). Source
  replacement, persistence and eviction are now specified by the headless concept
  review in that design (Review direction: one observation source, independent
  facts). Proposed: connection-established cache incarnation, non-reusing cache
  clock, captured dependency evidence and persisted last-known values. Implement
  this observation contract before field/answer separation and disconnected Move;
  no new product preference is inferred.
  The SQLite teardown repair is implemented; Move remains unbuilt. The three
  product questions remain open and independent of this engineering gap.

- Jack Heart replied “1a” to the 2026-11-15 conversation review's disconnected-edit
  question: save and project Complete/rename/move locally, show “Waiting for
  connection,” and deliver on reconnect to the original Machine. Retain across
  restart; recover uncertain sends without duplicate execution. Disconnected Move and the remaining cross-platform cutover are unbuilt.

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

- Retired imported/editor-linked creations keep only request/draft identities,
  a canonical command digest and retirement time in the same SQLite owner, outside
  indexed pending reads. Legacy files use their modification date as submission
  time. Source rewrites keep that compact identity; there is no release lifecycle.
  This headless engineering choice trades roughly 200 bytes per phone-created Task
  for deleting cross-file coordination; it is not a recorded preference.

- Creation action labels, headless reversible choice from the 2026-11-15
  concept-review: one vocabulary on both hosts, replacing Desktop "Retry
  recovery" and phone "Check status" for the same call. Built: **Try again**
  (never saved), **Edit request**, **Check request** (saved, outcome unknown),
  **Inspect** (receipt needs attention). Reopening belongs to attachment. Copy only; nobody has
  stated a preference. Request rows currently sit above the Task list on both
  hosts; their position has not been discussed.

- Phone reopen label, noticed in the 2026-11-15 headless concept review: the
  creation cut changed the phone Session's disconnected card from "Reconnect" to
  "Retry opening" for every Session by reusing the creation action's label. The current
  cut restores "Reconnect" there (existing copy) and keeps Desktop's "Retry
  opening". Whether both hosts should use one word, and which, is undiscussed.

- The portfolio's Waves status strip retains its prior non-creation scope and
  renders the shared pending-command row. Creation destinations and their editors
  live in repository windows; the Task navigator and phone feed list all kinds.

- Request rows, headless reversible choices from the 2026-11-15 concept review of
  `e010cda519`; nobody has stated a preference on any of the three:
  - A routine change being delivered normally (Complete, rename, move) shows no
    request row, only its projected result. Read from “completing a task should
    make it disappear immediately” and “a lot of intermediate states”; the prior
    cut listed a “Complete Task · LOO-12” row until settlement. The current cut
    suppresses that row while delivering.
  - Waiting for connection shows one row per held change, reusing the existing
    row. One summary line (“3 changes waiting for connection”) is the undiscussed
    alternative.
  - Abandon takes Complete's next-Task rule (next in list order, wrapping, else
    empty). The prior Desktop Abandon picked next, then previous, then the Wave; the
    current cut uses the shared next-Task rule.

- Open product questions from the 2026-11-15 headless concept review of the
  healthy-change/reconnect cut. Nobody has answered either; the built behavior
  stands meanwhile. They are numbered in `scratch/concept-review.html`.
  - Abandon while disconnected. The cut saves Abandon like Complete and delivers
    it on reconnect, however much later. The "1a" option named Complete, rename
    and move; Abandon closes PRs and removes branches. Alternative: Abandon needs
    a reachable Machine and says so.
  - A change waiting for connection has no way back. Its row offers no action,
    so a mistaken Complete or Abandon made offline will be sent. A change never
    sent could offer Cancel, removing the saved request and its projection; one
    already sent could not.
- Headless reading of "rename" in the 1a answer: the renames the apps offer
  (Session, Wave, Dashboard). No command edits a Task title.

- Headless reversible choices from the 2026-11-15 concept review of the
  conversation/Machine-link cut; nobody has stated a preference on either:
  - The phone feed shows one "Waiting for connection" line for its Machine, and
    held changes list their titles under it without repeating the words. The current pass implements that presentation.
  - Offline Session rename is built on Desktop. The phone shows a Task title for
    each conversation and has no rename; whether it should show Session names
    or offer rename is undiscussed (question 3 in `scratch/concept-review.html`).

- Headless rename choice, 2026-11-15: a newer name edit replaces an older rename
  that never saved, including when its save fails after the newer gesture. The
  failed request cannot later retry behind the newer edit. Saved renames retain
  their IDs and serialize by Session until the earlier domain outcome is known.
  Stamped name readings and the rename stage's actual name join one field owner;
  their sessions revision orders them across query identities. A name result can
  retire independently of membership queries because it carries that field value
  and fence; it never fabricates a Machine query stamp.

- Rename targets retained Session metadata, including a completed conversation;
  it does not require a live input attachment. The domain transaction still owns
  naming permission, validation, the actual stored name and definite refusal.

# Remaining design choices — LOO-366

Jack Heart concluded review before the October 5 implementation attempt. The
[design](keep-every-wave-ready-for.md) owns accepted decisions, implementation
status and evidence; earlier review notes remain at
`6dc8536fedce0852bd4931682c7ffb0a871cf17d:scratch/questions.md`.

- Preservation mechanics need no product decision. Acceptance retains shared
  acquisition guards through SQLite commit; cold detail re-reads ownership under
  its guard, and reteam uses full readbacks and authorized Team reconciliation.
  The prior iteration’s reteam repair request is satisfied locally;
  hierarchical checkout admission now excludes registration at an unregistered
  missing ancestor, preserving sibling progress. Its Task-population scan is gone.
  Accepted registration facts and the Wave planning boundary remain next; rotation
  still holds no checkout exclusion. The rejected global guard remains in history.
  Configuration-switch and created-successor recovery proofs depend on the later
  binding/transition replacement; name selection cannot recover preserved names.
- Chapter metadata representation and projection. Cross-Wave historical inspection
  remains proposed; Wave-scoped chapter creation is requested.
- Skill/Flow composition and the retained Task-candidate format between KR planning,
  chapter creation and separate Task admission.
- Designated fixture Waves and provider write authority beyond Intelligence.
  Jack Heart selected its exact Project for autonomous repair; the design retains
  that authority and the unproved Backlog/empty-Flow acceptance boundary.

Shared local binding ownership and the bounded historical name-only cutover are
resolved. Neither decision establishes implementation or installed acceptance.

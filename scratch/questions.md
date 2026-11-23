# Open questions

1. Work-session Start skips its steer claim when the server answered and the
   Session is interactive, decided in the hook from the store, not from a new
   `SessionBoundaryAnswer` field (released hooks reject unknown fields).
   Passed over: an answer field `wakes: bool`. Revisit with LOO-838's answer
   cleanup.

2. Codex's first boundary is reported by lf's launch (`open_native`) with a
   SessionStart-shaped callback, because Codex fires SessionStart only inside
   a turn. Passed over: a new `BoundaryMoment::Open` (wire change for one
   caller) and a store-revision domain the watcher reads for new addresses
   (migration plus triggers; would delete `After::Reachable`).

3. Claude launched with an opening prompt and a waiting steer may run two
   turns (prompt, then the woken steer). Steers arrive once. Left as is.

# Open questions

1. Work-session Start skips its steer claim when the server answered and can
   wake this conversation (an idle candidate on this AgentProcess), decided
   in the hook from the store. Passed over: an answer field `wakes: bool`
   (released hooks reject unknown fields; revisit with LOO-838's answer
   cleanup), and `interactive` alone, which leaves a steer for a Claude with
   no published inbox until the first typed turn.

2. Codex's first boundary is reported by lf's launch (`open_native`) with a
   SessionStart-shaped callback, because Codex fires SessionStart only inside
   a turn. Passed over: a new `BoundaryMoment::Open` (wire change for one
   caller) and a store-revision domain the watcher reads for new addresses
   (migration plus triggers; would delete `After::Reachable`).

3. Claude launched with an opening prompt and a waiting steer may run two
   turns (prompt, then the woken steer). Steers arrive once. Left as is.

4. Concept review (`scratch/concept-review.html`): Codex's launch report
   keeps its Codex-shaped payload with a truthful `source`. Passed over:
   making "a conversation became idle" a store revision the watcher reads,
   which would delete `After::Reachable` and the synthetic report but adds
   triggers on `session_activity` (written every turn) and a Machine-wide
   idle scan per turn end. A separate Task if wanted.

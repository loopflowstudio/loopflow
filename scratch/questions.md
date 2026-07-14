# Open questions — W2-76

**Assumption (scope, resolved by executive decision):** the deletion W2-76 asks
for already landed in #872 — no rotation code, wire route, or Swift caller
remains. Rather than close the task with an empty PR (design-only landings are
not real product change), it ships the part of the removal that never reached
the caller: retired `lf op *` spellings now name their replacement instead of
failing as a missing skill. If the supervisor wanted a strict no-op closure
instead, the PR is one commit to drop.

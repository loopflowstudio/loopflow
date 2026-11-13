# LOO-874 demo decision, 2026-11-13

No hands-on demo happened: this run is headless and nobody is available.
No reactions or approval were collected. No demo processes were launched.

Decision: return to implement (built; see "Target check presentation" in `scratch/questions.md`). Second-round QA demonstrated fresh-target moves
and repaired recovery commands, including retrying from the target. Its remaining
target-side status gap is concrete: mini records a `checked` move but status
shows `Unowned` / `Not checked out` with no way to continue it.

Show the recorded check, its source and time, and a retry command using the
target's own name. Describe a past check rather than asserting a live incoming
move: a later claim-back on the source may have superseded it. Retrying already
consults the source. Keep ownership and checkout facts separate. Exercise both
a pending arrival and a source claim-back so stale checks never imply ownership.

This is an autonomous decision from the recorded QA results, not feedback
attributed to Jack Heart. Gate's remaining checks stay with gate.

## Decision pass 2, 2026-11-13

Advance. No hands-on demo happened: this run is headless and nobody is
available. No reactions or approval were collected; no demo processes started.

The previous pass required the target to show the recorded check, source, time
and a retry using its own name, without implying ownership. Third-round QA
(`scratch/qa.html`, branch bbd78b36df plus the repair committed as ab07d36f42)
observed those lines, ran the printed retry through arrival, and observed the
check line disappear. It also ran the previously code-only source claim-back
path: retry asked the source, began a new move, and carried the edit across.
QA repaired the unreachable-source message and recorded two focused tests passing.

Remaining wording: retry says "Carrying on" even when the source starts a new
move after claim-back. This does not reopen the prior gap: status describes a
past check and the printed command requests a move to this Machine; the source
resolves it against current state. No further implementation loop selected.
Provider conversation travel, Linear people and Desktop remain outside this
headless QA observation; wider checks remain with gate. This decision records
the autonomous judgment for this step, not a person's reaction or approval.

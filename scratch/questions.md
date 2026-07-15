# Open questions — W2-155 (give every wave its own Linear team)

## Renumbering on team move (decided, proceeding)

Linear reassigns the issue **number** when an issue moves teams — `W2-155` →
`PRD-<next>`, not `PRD-155`. A dry run cannot predict the number (Linear assigns
at move time). The user story's "`PRD-123` because `W2-123`" is therefore not
achievable by a team move.

**Assumption I'm proceeding on:** forward-cut + bounded migration.
- New work goes to the per-wave team from `pm init` onward (delivers the story).
- Open, *settled* issues migrate with a traceability comment (`was W2-155`) and a
  snapshot re-sync; **active/in-review Sessions are deferred**, never moved.
- Completed `W2` history stays in the shared team as historical (shipped PR/commit
  references to `W2-N` are immutable; renumbering them orphans those references
  for no gain).

If number preservation is actually required, the only mechanism is
delete+recreate with pinned numbers — which destroys issue UUIDs and thus Task/
Project Session ownership. Rejected as strictly worse than renumber+traceability.
Flag for Jack if the historical-number continuity matters more than Session
integrity.

## Scope of the settled-open migration set (decided)

`reteam` migrates only open issues under the wave's Initiative Projects that have
no non-terminal Session. This Task (W2-155) is itself running under the product
Initiative, so PR 2's migration will **defer W2-155** — it can only be moved
later from a clean context after this Task completes. That's intended, not a gap.

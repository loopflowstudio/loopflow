# Open questions — W2-155 PR 2 (reteam existing work)

## Renumbering on team move (DECIDED — directive v2 confirms)

Linear reassigns the issue **number** on a team move (`W2-155` → `PRD-<next>`,
not `PRD-155`). Directive v2 makes this binding: **never promise or implement
`W2-N → PRD-N` renaming.** We preserve stable UUID + a traceability comment
(`was W2-155`), not the number. If number continuity were ever required the only
mechanism is delete+recreate with pinned numbers — which destroys UUIDs and thus
Session ownership. Rejected as strictly worse.

## Completed W2 history (DECIDED — preserve as historical)

Completed issues stay in the shared W2 team. `reteam` moves only open, settled
issues. Shipped PR/commit/MEMORY references to `W2-N` are immutable; renumbering
them buys nothing.

## Migration set (DECIDED)

`reteam` moves open issues with no non-terminal Task Session
(`TaskSessionStatus::is_terminal()` = Completed|Abandoned). W2-155 itself is
Running under the product Initiative, so PR 2 **defers W2-155** — intended, not a
gap. Idempotency = skip issues whose identifier already carries the target team
key.

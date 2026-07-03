# Open questions — jack-heart.lf-loop

## Asana roadmap reconciliation is owed (blocked this run)

The stored Asana token was **expired** during this headless run, so the roadmap
could not be read or mutated. Full context — which project is canonical, the
lf wave follow-ons to file, what to close — is in `wave/goals/MEMORY.md` →
"Roadmap reconciliation owed (Asana)". Run `lf op auth asana`, then `lf op pm`
when a human can re-auth.

## Rebase: canonical Asana project (assumption — verify if wrong)

During rebase, `main` and this branch had independently bootstrapped `wave/goals/`
into two Asana projects. **Adopted main's project `1216257471889000`** (it's on
trunk; a rebasing branch conforms). The branch's duplicate `1216272792262792`
was abandoned. If `1216272792262792` was actually the live one, flip
`wave/goals/GOAL.md`'s `asana_project` back and re-register the goals via
`lf op pm`. (Details of the per-file conflict resolution are in git history.)

## Scope: progress arm only (by design)

The design specifies four arms in one branch; this branch shipped the smallest
shippable slice — the `lf wave` progress arm. Monitor, cron, and chat are
tracked as follow-ons in `wave/goals/MEMORY.md`.

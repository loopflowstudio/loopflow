---
asana_id: '1214270115637593'
---
# PM round-trip

**Finish line:** Asana is always a mirror of wave and PR reality. `needs:` declarations round-trip as Asana dependencies; PR events auto-update Asana tasks; a scripted delete-team + re-init rebuilds PM cleanly after reorgs.

## Context

Pull, push, and export work for Asana. The configured team is the canonical wave set. Task descriptions preserve markdown via `html_notes`. Missing pieces:

- **Dependency sync** — `needs:` declarations on roadmap items round-trip with Asana dependency graphs
- **Lifecycle sync** — PR opened / failed / merged triggers Asana comments and completion on the corresponding task
- **Reset tooling** — `lf op pm delete-team` + scripted reset flow so a repo reorg can fully rebuild PM state in one command

## Daily experience

You add `needs: [1-foo]` to a roadmap item. The matching Asana task links to its dependency. Open a PR touching that item; the task gets a "PR open" comment. Merge; the task completes in Asana. Separately, after a big wave reorg, one reset command wipes and rebuilds PM state from the canonical team in under a minute.

## Done when

- Dependency graphs in Asana reflect `needs:` declarations on roadmap items
- PR open / merge / failure events feed PM comments and completions
- `lf op pm delete-team` deletes the configured team with a confirmation guard
- Reset script chains delete-team → init → push-diff as one idempotent operation
- Reset survives a wave reorg — renamed / added / removed waves all land cleanly

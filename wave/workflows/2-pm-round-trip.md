---
asana_id: '1214270115637593'
---
# PM round-trip

**Finish line:** Asana state is always a mirror of wave/PR reality. `needs:` declarations round-trip as Asana dependencies; PR events auto-update PM items; a scripted delete-team + re-init rebuilds PM cleanly after reorgs.

## Context

Pull / push / export work against Asana. Priority buckets map to Asana's Priority field; descriptions preserve markdown via `html_notes`. Missing pieces:

- **Dependency sync** — `needs:` declarations on roadmap items round-trip with Asana dependency graphs
- **Lifecycle sync** — PR opened / failed / merged triggers Asana-side comments and completion on the corresponding PM item
- **Reset tooling** — `lf op pm delete-team` + scripted reset flow so a repo reorg can fully rebuild PM state in one command

## Daily experience

You add `needs: [1-foo]` to a roadmap item. The Asana graph updates. Open a PR touching that item; the PM item gets a "PR open" comment. Merge; the item completes in Asana. Separately, after a big wave reorg, one reset command wipes and rebuilds PM state from `wave/` in under a minute.

## Done when

- Asana dependency graphs reflect `needs:` declarations on roadmap items
- PR open / merge / failure events feed PM comments and completions
- `lf op pm delete-team` exists with a confirmation guard
- Reset script chains delete-team → init → push-diff as one idempotent operation
- Reset survives a wave reorg — renamed / added / removed waves all land cleanly

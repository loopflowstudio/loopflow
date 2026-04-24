---
asana_id: '1214270115637593'
---
# PM round-trip

**Finish line:** PM state (Asana / Linear / Notion) is always a mirror of wave/PR reality. `needs:` declarations round-trip as provider dependencies; PR events auto-update PM items; a scripted delete-team + re-init rebuilds PM cleanly after reorgs.

## Context

Pulls / push / export work for Asana, Linear, Notion. Priority buckets translate to provider-native vocabulary. Notion item descriptions sync as real pages; Asana preserves markdown via `html_notes`. Missing pieces:

- **Dependency sync** — `needs:` declarations on roadmap items round-trip with provider dependency graphs
- **Lifecycle sync** — PR opened / failed / merged triggers provider-side comments and completion on the corresponding PM item
- **Reset tooling** — `lf op pm delete-team <provider>` + scripted reset flow so a repo reorg can fully rebuild PM state in one command

## Daily experience

You add `needs: [1-foo]` to a roadmap item. Asana graph updates. Open a PR touching that item; the PM item gets a "PR open" comment. Merge; the item completes in Asana. Separately, after a big wave reorg, one reset command wipes and rebuilds PM state from `wave/` in under a minute.

## Done when

- Dependency graphs in Asana / Linear / Notion reflect `needs:` declarations on roadmap items
- PR open / merge / failure events feed PM comments and completions
- `lf op pm delete-team <provider>` exists for all three providers with a confirmation guard
- Reset script chains delete-team → init → push-diff as one idempotent operation
- Reset survives a wave reorg — renamed / added / removed waves all land cleanly

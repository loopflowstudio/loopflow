# PM reset ops

**Finish line:** Full reset of PM state is a scripted two-step: `lf op pm delete-team <provider>` wipes the provider-side project/team and supporting state, then `lf op pm init --all` re-bootstraps from `wave/` with the new structure. Rinse and re-run cheaply.

## Context

PM state drift happens when waves get renamed, reshaped, or deleted. Item-level primitives exist (`lf op pm list`, `delete-task`, `delete-project`), but there's no team/workspace-level delete. After a big reorg (e.g. 2026-04-24), the clean path is "burn the old Asana team and re-init from scratch" — that requires a team-level delete plus a predictable init/reset script.

## What to build

1. **`lf op pm delete-team <provider>`** — delete the project/team the current config points at, with confirmation. Provider-specific but same surface shape across Asana, Linear, Notion.
2. **Reset script** under `scripts/` — one command to: confirm, delete provider team, re-init from `wave/`, push-diff. Guard-railed so a typo can't nuke a live customer project.
3. **`lf op pm init` hardening** — `--fresh` flag that fails if a conflicting team/project already exists, so init+reset are distinct modes.

## Constraints

- Never run destructive team-delete without explicit confirmation (interactive or `--yes` flag).
- Reset script must work across a wave reorg — renamed/deleted/created waves all land cleanly.
- Dry-run mode that previews what would be deleted.

## Done when

- `lf op pm delete-team` ships for Asana, Linear, Notion.
- Reset script under `scripts/` that chains delete-team → init → push-diff.
- Documented in README under PM ops.

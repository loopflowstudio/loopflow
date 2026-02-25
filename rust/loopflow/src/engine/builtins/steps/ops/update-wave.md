---
requires: diff vs main, wave/<wave>/, scratch/
produces: wave/<wave>/ (updated), scratch/ (promoted files removed)
---
Reconcile wave state after work.

## Goal

After a build, `wave/<wave>/` should reflect reality:

- Completed work is marked accurately
- New actionable follow-ups from `scratch/` are promoted
- Duplicates are merged (never silently overwritten)
- Promoted scratch files are removed

## Workflow

1. Read the diff to understand what was actually built.
2. Read `wave/<wave>/README.md` and roadmap items in `wave/<wave>/`.
3. Update wave status/roadmap files based on what shipped.
4. Review `scratch/` for unfinished or actionable artifacts.
5. Promote actionable items into `wave/<wave>/`.
6. If destination files already exist, merge/dedupe content intentionally.
7. Remove scratch files that were promoted.
8. If `wave/<wave>/MEMORY.md` exists, distill stable observations into canonical docs and trim duplicated memory entries.

## Promotion rules

- Promote work items that represent clear next actions.
- Keep one canonical copy in `wave/<wave>/`.
- If a destination exists, merge content; do not clobber existing files.
- Skip disposable notes that are already captured elsewhere.
- If memory observations were promoted into canonical docs, `MEMORY.md` should not keep duplicated long-form copies.

## Output

Updated files under `wave/<wave>/` plus cleanup of promoted `scratch/` files.

If no wave changes are needed, leave a short note in the commit message: `wave: reviewed, no changes needed`.

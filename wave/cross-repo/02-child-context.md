# 02: Child-Aware Context Loading

## What to build

lf queries lfd for children at session start and uses that to resolve child repo paths. When areas reference children, load docs from the child repo.

## Key functions

- `resolve_children(lfd_client) -> HashMap<String, PathBuf>` — Query lfd for child name→path mapping.
- Extend `gather_area_docs` to handle paths in child repos.
- Extend `gather_context` to optionally include child root docs.

## Behavior

1. At session start, lf calls `GET /repos/{id}/children` to get child repos.
2. Child repos are available by name (derived from repo directory name or portfolio display name).
3. Area paths referencing children resolve against the child's repo root.
4. When a child area is active, doc loading walks the child's path hierarchy for `.md` files, same as local areas.
5. Child docs share the area budget.

## Constraints

- If lfd isn't running, no children are available. Fail gracefully — session works as single-repo.
- Child repo must exist on disk at the path lfd reports. If not, warn and skip.
- Doc loading from children uses the same walk-up-parents logic as local docs.

## Done when

- lf queries lfd for children and gets back name→path mapping
- Docs from a child repo load into session context
- Session works normally when lfd is unavailable (no children, no error)

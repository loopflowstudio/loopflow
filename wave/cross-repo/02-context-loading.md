# 02: Bidirectional Context Loading

## What to build

lfd auto-resolves related repos and injects their areas into session context. This makes cross-repo context loading seamless — no manual path spelling.

## Key insight

lf already accepts arbitrary area paths. `lf implement -a /other/repo/src/` works today. There's no lf-level change needed for cross-repo context — lf takes paths, period.

lfd's job is making this automatic. When lfd creates a session, it looks up related repos from the edge graph and adds their areas to the session's context. The agent doesn't need to know filesystem paths — lfd resolves `RepoId` → path from the store.

## Key change

Context loading happens inside lfd (`gather_context` → `gather_documents`), not via lf querying an API. lfd already has access to the store — it looks up related repos directly.

No new HTTP endpoints needed for this stage. The children/parents endpoints from stage 01 serve Concerto, not the context pipeline.

## Key functions

- `resolve_related_repos(store, repo_id) -> RelatedRepos` — Query store for parents and children.
- Extend `gather_documents` to include docs from related repos.
- Extend `GatherContextOpts` with resolved related repos.

```rust
struct RelatedRepos {
    parents: Vec<(RepoId, PathBuf)>,   // R access
    children: Vec<(RepoId, PathBuf)>,  // RW access
}
```

## Behavior

1. At session creation, lfd resolves the session repo's `RepoId` and queries the store for edges.
2. For each related repo, resolve the local filesystem path from the store.
3. Area paths referencing related repos resolve against that repo's root.
4. Doc loading walks the related repo's path hierarchy for `.md` files, same as local areas.
5. Related repo docs share the area budget.

## Constraints

- If lfd has no edges for the repo, context loading is unchanged. Single-repo sessions are unaffected.
- Related repo must exist on disk at the path lfd reports. If not, warn and skip.
- Doc loading from related repos uses the same walk-up-parents logic as local docs.
- Parents contribute read-only docs. Children contribute docs that may be edited.

## Done when

- Parent sessions see child repo docs in context
- Child sessions see parent repo docs in context
- Context loading works normally when no edges exist
- Missing-on-disk repos are warned and skipped, not fatal

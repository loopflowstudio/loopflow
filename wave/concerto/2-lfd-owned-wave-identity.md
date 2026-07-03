---
asana_id: '1216257471655534'
---
# lfd-owned wave identity

**Finish line:** A wave's GOAL and MEMORY are a single master copy owned by lfd,
outside any repo — editable, personal, the source of truth. Repos hold
materialized projections that reconcile to the master: export on PR (merged with
the repo's in-repo copy), pull-in on subscribe. The wave home is the directory
the goal-loop harness and yazi root at.

## Context

The `Wave.repo → repos: [RepoWork]` split shipped: identity now lives at the
wave level and execution state per repo, across Rust store/DTOs, Python, Swift,
and fixtures. What's *not* done is making GOAL/MEMORY a single lfd-owned master —
today they still live per-repo-worktree, which is the root of the "goal not
found" gap in [[1-embedded-terminal-build-driver]] (launch resolves the goal from
a main-derived sibling that may not carry the file). This item makes identity
actually lfd-owned and syncable.

- **Master store** — lfd persists GOAL + MEMORY per wave, keyed by wave id,
  outside any repo. `wave_config.rs` already reads `GOAL.md` frontmatter; extend
  it to own the file as the master.
- **Wave home** — an lfd-managed directory (`wave/GOAL.md`, `wave/MEMORY.md`,
  `scratch/`) the harness cwd's into and yazi browses.
- **Export on PR** — on a PR to repo X, merge master GOAL/MEMORY with X's in-repo
  `wave/<name>/` copy; the PR carries the result.
- **Pull-in on subscribe** — subscribe to repos to pull their `wave/<name>/`
  edits back into the master.
- **Serialization** — file-as-master (yazi edits, lfd parses frontmatter).
  GOAL.md stays short (vision + metrics + milestones); MEMORY is read-on-demand
  when it won't fit, not compacted.

## Done when

- GOAL/MEMORY for a wave persist in lfd independent of any repo checkout.
- Creating a PR in a repo materializes the merged GOAL/MEMORY into that repo.
- Subscribing to a repo pulls its wave-file edits back into the master.
- The wave home directory is what the harness and yazi open.

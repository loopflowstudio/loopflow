# Resolve Wave definitions from the owning worktree, otherwise remote main

Jack's design review direction, 2026-09-29.
[Working design](resolve-tasks-from-linear-and.md),
[Wave mapping](wave-existence-and-linear-migration.md),
[open choices](questions.md).

## Selected behavior

Jack described the repository as the owner of Wave definitions, the local store
as Loopflow's operational owner, and Linear as a synchronized provider once a
mapping exists. Jack also requires separate answers for main and each worktree:
adding or deleting a Wave in a worktree must immediately change API results in
that worktree, including before commit. Without a clear owning worktree, use the
repository's main definition. Jack then specified that main should stay clean
and dirty main changes are disposable, and proposed using remote main throughout
instead of local main. The working design therefore uses the configured remote's
main tracking ref (normally `origin/main`) as the baseline. Jack accepted this shared-baseline rule and clarified that it is not a blanket
replacement of local main in unrelated Git operations. The review is approved;
remaining freshness and no-remote details are explicit below.

```python
def waves(repo, worktree=None):
    source = worktree if worktree is not None else remote_main_definition(repo)
    return local_store.waves_from(source)
```

This is a conceptual interface, not a new service or proposed persisted snapshot
table. The existing local planning interface owns the result. Import/reconcile
the relevant repository files before returning it; a stale store entry must not
hide a worktree addition or resurrect a deletion. A watcher can optimize reads,
but no explicit sync, commit, merge or provider response is required to see a
definition change locally.

- The selected worktree supplies its complete Wave definition set, including
  uncommitted file contents and newly authored Wave files. Do not union its set
  with main or fall back to main per missing Wave: that would undo deletions.
- A worktree with no Wave definitions returns an empty set. An unreadable or
  invalid definition is an error in that view, not grounds to switch to main or
  destructively reconcile a partial scan.
- Main remains independently addressable for context-free reads. An edit in
  worktree A cannot replace the main answer or worktree B's answer. A branch name
  alone is insufficient context: branch switches and dirty files can change the
  view at the same path. No private Task-plan copy is created for each view.
- Determine context once at the command/API boundary and use it for lists,
  detail, selection and subsequent actions. Existing execution with an owning
  checkout uses that checkout; genuinely context-free reads use main. Do not
  pick an arbitrary live worktree to supply a default.
- Removing a Wave from a branch removes it from that branch's available Wave
  set. It does not delete Linear records, Task history or another view's Wave.
  Ordinary worktree Flows remain usable. Missing definition and provider Task
  deletion remain different observations; do not globally invalidate execution
  from one checkout's file deletion.
- Wave definitions and bindings originate in repository files. Linear refresh
  cannot reintroduce a Wave omitted from the selected source. Connected Task and
  chapter planning retains the previously selected Linear authority; Jack's Wave
  definition clarification does not authorize a competing Git Task-plan writer.
- Importing a branch definition is local observation, not automatic publication
  or deletion in Linear. Explicit create/link/connect operations retain their
  provider effects. When an edited mapped definition should sync outward remains
  a policy choice; do not infer provider writes from an API read.

## Remote-main baseline and clean main

Resolve the default definition from the configured remote's main tracking ref,
not the local main branch or its dirty filesystem. `origin/main` is the ordinary
example, not an instruction to hardcode `origin` in repositories using another
remote. Read the tree at the resolved commit without checking it out or resetting
files. Keep the exact source ref/commit available as observation provenance.
Use the same baseline resolver wherever these APIs need a main definition.

Proposed freshness policy: use the locally fetched remote-tracking ref for ordinary
reads; normal Loopflow fetch/rebase refresh advances it. This is last-fetched
remote state, not a live claim about GitHub. Do not add network I/O to every read
or fall back silently to unpublished local main commits after a failed fetch.
Explicit feature worktrees still use their actual files and dirty changes.

Jack's main policy is to avoid authoring there and normally discard dirty main
changes. Such changes must never become the default Wave definition. A read
ignores those bytes; it does not reset or delete files. Placement and any actual
cleanup remain owned by Loopflow operations. Do not treat this design discussion
as a request to clean the supplied worktree or change its Git state.

Still specify behavior for a repository without a remote/main tracking ref and
whether an explicitly supplied main checkout should be normalized to the remote
baseline. Local-only planning must remain usable; feature-worktree reads do not
require the remote baseline. Exact fetch cadence is not selected by this review.

## Remaining implementation choices

The local store can derive or cache contextual imports; avoid a second durable
Wave registry or independent planning writer per worktree. Preserve existing Wave
IDs and provider bindings through contextual reads. Resolve rename identity and
conflicting edits without taking the last-inspected worktree as repository truth.

## Existing source and proof to add

`ops/pm.rs::list_local_waves(repo)` already recursively discovers directories
containing `GOAL.md` from its supplied path. Conversely,
`canonical_wave_title_path_with_store` requires registered ancestry for nested
Waves and resolves the canonical repository while reading it. This is partial
source evidence, not proof that current APIs consistently respect worktree files.
Reuse file discovery and remove reader prerequisites that contradict the selected
context; do not introduce another naming or discovery implementation.

Use one disposable repository whose fetched remote main contains `A` and `B`. In worktree X,
delete `A` and add `C` without committing. Assert X returns `B,C`, an unchanged
worktree Y returns `A,B`, and the context-free main view returns `A,B`, regardless
of read order through the same local store. Revert the edits and prove immediate
restoration. Include modified goals, nested `A/B`, empty and invalid definitions,
branch switching and a mapped Linear record that must not restore a locally
deleted Wave. Assert no provider mutation, Task deletion or new execution from
these reads. Add dirty files and unpublished commits on local main and prove
that neither changes the default remote-main answer. Advance the remote, show
that the default stays at the last fetched commit until refresh, then updates.
No read may clean/reset the checkout. Exercise the API and public CLI, not only filesystem discovery.

No implementation, behavioral tests or provider writes occurred in this review.

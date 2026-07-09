# Open questions

## Blocker: rebase aborted — another agent owns this worktree

`lf rebase` (skill: rebase) was asked to rebase `jack-heart/lfapi` onto
`origin/main` (1 commit behind, 21 expected conflicts). It did not run.

A second agent is live in this same worktree:

```
21654  node .../bin/codex exec -C /Users/jack/src/loopflow.lfapi
```

It is doing the same class of work this branch does — sweeping `lf op ...`
mentions out of docs and prompts. Its edits are uncommitted and still growing:
the dirty-file count went 1 → 8 → 14 over ~3 minutes, and the last write landed
77s before I checked, with the process still resident.

Rebase requires a clean tree. Every way to get one here destroys or desyncs
that agent's in-flight work:

- `git stash` yanks files out from under a process holding them in memory; its
  next write replays pre-rebase content over the rebased tree.
- Committing its edits attributes unreviewed, possibly half-finished work to
  this branch.

So the rebase is deferred rather than forced. Nothing was stashed or reverted.

### What I did change

One commit, `71dc8757 docs: drop ops.md entries for commands removed with lf op`.
When I started, `docs/ops.md` was the only dirty file, and I read it as a stale
leftover — it deletes the doc sections for `queue reconcile`, `next`, `advance`,
`doctor`, `sync`, and `shell`, all of which are genuinely gone from the clap
grammar in `rust/loopflow/src/lf/mod.rs`. I committed it before discovering the
codex process.

That was almost certainly codex's work, not a leftover. The commit is
content-preserving — the file on disk is byte-identical to what codex wrote, so
its editing is unaffected — but the authorship is now mine. If codex intends to
commit that hunk itself, drop `71dc8757` and let it.

### To finish the rebase

Once pid 21654 exits and `git status` is clean:

```bash
git fetch origin main
git rebase origin/main
```

Expect conflicts in ~21 files. `origin/main` is 1 commit ahead
(`601881cd waves: reshape roster into wave/project/task ontology`), plus
`3ddf006c install.py: drop stale --global from lf op sync-skills` — which
touches `docs/lfop.md`, a file this branch renamed to `docs/ops.md`. That
rename/edit collision is the one conflict worth reading carefully; take this
branch's `docs/ops.md` and fold in main's `sync-skills` correction if it still
applies (this branch removes `sync-skills` from `install.py`, so it may not).

For the rest: files central to this branch's intent (the `lf op` → first-class
command sweep — `lf/mod.rs`, `bin/lf.rs`, `ops/*`, goldens, builtin skill
prompts) keep this branch's version. Files main touched incidentally take
main's.

## Pre-existing: stale claim in scratch/lfapi-review.md

`scratch/lfapi-review.md` says the implementation "rehomed" `next`, `advance`,
`branches`, `sync`, `doctor`, `sync-skills`, and `shell` as top-level commands
and asks reviewers to confirm. That is no longer true for most of them — only
`sync-skills` survives in `lf/mod.rs`; the rest are deleted, matching
`scratch/lfapi-design.md`. The review doc should be corrected before it informs
a review decision.

# Stale PR bases — lf-cb9012a

Jack Heart requested autonomous investigation and repair through demo and PR review
on 2026-10-11; publish, do not land. This implementation uses the Task brief as
the design. No existing scratch design was supplied.

## Outcome and decision

`lf pr open` derives the comparison base from current HEAD and the selected
upstream (main or the live stack parent), refreshes root placement and publishes
without integration. A saved base is a cached comparison, not evidence that any
commit is foreign. Keep all branch commits, including inherited dependencies;
review the actual GitHub range. No rebase or reset is a publication repair.

Delete — do not maintain (removed):
- `verify_task_pr_range_mode`'s saved-base ancestry classification and unbounded
  commit/file diagnostics; preserve current-branch and managed-Task ownership.
- `was_published_tip` and its reflog exception; current ancestry suffices even
  after reflog expiry or garbage collection of the old base.
- Tests requiring refusal solely for inherited/divergent saved bases. Replace
  with command-level publication, range parity and unchanged-history proofs.

Keep empty-range refusal, unrelated-history failure, fresh upstream acquisition,
stack selection and post-push placement timing for explicit sync. No migration.

October 11 implementation review found a counterexample to unconditional saved
base replacement: a stacked child's base is also the comparison fork used after
parent squash. Healing it backward during publication can restore a dependency
main removed. The revised design retains stacked placement until explicit sync,
while resolving publication's current range independently on every call. Root
placement refreshes immediately. This uses existing fields and owners; no second
stored base. The regression publishes a squash-parent child, then syncs and proves
main's deletion survives alongside child edits.

## Five Whys and counterevidence

1. **Why could the PR not open?** `create_or_update_pr` calls
   `verify_task_pr_range` before push (now `refresh_task_pr_base`). The old gate compared saved B with merge-base M,
   refusing when neither was an ancestor. The actual upstream/HEAD merge-base
   already exists; no history rewrite is necessary to identify the range.
2. **Why was B stale?** Task preparation saves its initial fork in placement
   (`prepare_task_worktree`, `sync_task_placement`). Etude HEAD reflog starts at
   `246cc8496902` at 00:36:47 PDT. Later raw Git movements do not update SQLite.
   On October 11, origin/main fetched `0fcdec5` (01:03:17), then `3da3c7d`
   (01:04:11), then recorded a push of `7830b3f` (01:13:33). That commit states
   PR #1/#4 history was rewritten to remove generated build/dependency output;
   it has the corrective merge's tree. Commit author identity is not evidence
   of who executed the push. The initiator beyond this commit account is unknown.
3. **Why did it refuse instead of adapt?** #924 (`7eb5e34c62`) encoded B as an
   ownership boundary to prevent #877/#882's inherited commits entering PRs.
   It conflated a stored comparison with current authorship. No graph comparison
   proves which dependencies may be removed. Updating B to M preserves them all.
4. **Why wrong wording and huge output?** #989 (`d587eeca5f`) expanded the
   refusal with complete logs and path lists for M..B and B..M. Two-dot file
   diffs compare endpoint trees, so reversing them repeats paths. “No common
   lineage” was false: Etude B and main share `7f06283`. The printed rebase
   used B as a replay cutoff without proving that its dependencies were on main.
5. **Why did previous repairs miss this?** #1459 (`832c01e862`, Wave memory's
   Transient recovery) repaired sync onto the PR's own branch, but only allowed
   stale B when its exact SHA appeared in that branch's remote reflog. #989's
   `submit_refuses_divergent_ancestry_naming_both_sides` explicitly tests a
   successfully rebased branch and requires refusal. These reinforced the gate
   rather than making Git the comparison authority. LOO-408 / #1488
   (`cead4c952`) removed historical execution vetoes from completion, and
   LOO-326 allowed cached planning for continuation; neither changed this gate.

Read-only evidence **corrects the supplied morning account**: the current Etude
reflog records a branch reset to `3da3c7d` at 01:12:31, a rebase onto `7830b3f`
at 01:14:41, then commits `bbee1c3` and `6ca0a7d` (07:35:09). Thus the incident
was not just main moving under an untouched prototype branch. The old prototype
SHA survives in history while the live branch was deliberately replaced. This
explains the divergent gate specifically. No Etude files, refs or records were
changed during this investigation. Installed repair remains unproved.

## Other stale-record refusal paths (source inventory)

| Path | Evidence and disposition |
| --- | --- |
| Publication saved base | Reproduced by existing divergent fixture; replace with current merge-base. |
| Explicit sync post-integration range | Same shared gate; inherited sibling fixture now must retain commits and heal. |
| Squash-parent `validate_fork_base` | Saved fork no longer ancestor can refuse before comparison merge; preserving unpublished edits requires a valid comparison. Bound diagnostic; no guessed replay command. |
| Task branch / active PR branch mismatch | `resolve_task_pr_base`, `task_pr_context`, `attach_task_github_pr`; source only. Identity reconciliation is separate from comparison-base repair. |
| Live stack parent's missing branch/record | `resolve_task_pr_upstream`; source only. Missing evidence cannot select main and silently change dependency. |
| Sync ownership receipt after raw Git | Sync continuation validates pinned branch/HEAD; source only. Stale unlocked receipts are adoptable; live locks remain authority. |
| Saved GitHub head / merge request | `request_task_pr_merge`, `invalidate_stale_merge_request`; source only. Fresh provider head must fence settlement, never inherit an old approval. |
| Absent checkout / placement collision | Task preparation retains identity and requires explicit placement recovery; source only, no relocation in this change. |

## Remaining

Gate owns affected suites, including stacked delivery; demo/review owns judgment
of the full preserved PR range. Publication remains the caller's later Flow step;
do not land. Installed Etude recovery and hosted GitHub acceptance are unproved.

`uv run python scripts/demo_pr_base.py` runs the real `lf pr open` in disposable
repositories, with external network denied and GitHub simulated. It proves new
PR creation and existing PR updates against main, preserving HEAD/tree/every
commit for rebased, inherited, force-rewritten-main, squash-parent and missing
saved bases. The separate stacked regression preserves sync's comparison fork.

Review finding fixed: publication's comparison must not replace a stack's
integration fork. Removed the redundant reread of saved placement for emptiness.
Compression shares upstream acquisition between root and parent branches, names
placement writes explicitly with `BaseUpdate`, and uses one tree comparison for
both zero commits and commits with no changes. Review found the old quiet-diff
status treated Git errors as changes; comparing tree IDs propagates read failures. Stack forks and post-push timing
remain unchanged; no deletion targets remain.


Checks: network-isolated `cargo test -p loopflow --test task_pr_range_tests` (13 passed); `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, demo-script Ruff passed; prior focused unsafe-stack sync pass remains applicable; affected-suite gate/hosted CI deferred to their Flow steps.

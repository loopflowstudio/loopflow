# Stale PR bases — lf-cb9012a

Jack Heart requested autonomous investigation and repair through demo and PR review
on 2026-10-11; publication for review is accepted, landing is excluded. The Task
brief supplies acceptance. Implementation at `ec3cd1179d`, integrated with main
at `131e8caa4c`, passed publication checks and awaits PR review; the approach below is implemented,
not yet accepted by Jack Heart. Reconciled 2026-10-11.

## Outcome and decision

`lf pr open` derives the comparison base from current HEAD and the selected
upstream (main or the live stack parent), refreshes root placement and publishes
without integration. A saved base is a cached comparison, not evidence that any
commit is foreign. Keep all branch commits, including inherited dependencies;
review the actual GitHub range. No rebase or reset is a publication repair.

Removed:

- `verify_task_pr_range_mode`'s saved-base ancestry classification and unbounded
  commit/file diagnostics; current-branch and managed-Task ownership remain.
- `was_published_tip` and its reflog exception; current ancestry suffices even
  after reflog expiry or garbage collection of the old base.
- Tests requiring refusal solely for inherited/divergent saved bases, replaced
  by command-level publication, range parity and unchanged-history proofs.

Empty-range refusal, unrelated-history failure, fresh upstream acquisition,
stack selection and post-push placement timing for explicit sync remain. No migration.

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
   proves which dependencies may be removed. Comparing against M preserves them all;
   stacked placement retains B for integration.
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
| Publication saved base | Reproduced through real CLI; current merge-base now permits publication. |
| Explicit sync post-integration range | Reproduced; inherited sibling fixture retains commits and refreshes root placement. |
| Squash-parent `validate_fork_base` | Saved fork no longer ancestor can refuse before comparison merge; preserving unpublished edits requires a valid comparison. Diagnostic is bounded and tested; no guessed replay command. |
| Task branch / active PR branch mismatch | `resolve_task_pr_base`, `task_pr_context`, `attach_task_github_pr`; source only. Identity reconciliation is separate from comparison-base repair. |
| Live stack parent's missing branch/record | `resolve_task_pr_upstream`; source only. Missing evidence cannot select main and silently change dependency. |
| Sync ownership receipt after raw Git | Sync continuation validates pinned branch/HEAD; source only. Stale unlocked receipts are adoptable; live locks remain authority. |
| Saved GitHub head / merge request | `request_task_pr_merge`, `invalidate_stale_merge_request`; source only. Fresh provider head must fence settlement, never inherit an old approval. |
| Absent checkout / placement collision | Task preparation retains identity and requires explicit placement recovery; source only, no relocation in this change. |

## Implemented evidence and review findings

`uv run python scripts/demo_pr_base.py` runs the real `lf pr open` in disposable
repositories, with external network denied and GitHub simulated. It proves new
PR creation and existing PR updates against main, preserving HEAD/tree/every
commit for rebased, inherited, force-rewritten-main, squashed-upstream and missing
saved bases. The separate stacked regression preserves sync's comparison fork.

Review finding fixed: publication's comparison must not replace a stack's
integration fork. Removed the redundant reread of saved placement for emptiness.
Compression shares upstream acquisition between root and parent branches, names
placement writes explicitly with `BaseUpdate`, and uses one tree comparison for
both zero commits and commits with no changes. Review found the old quiet-diff
status treated Git errors as changes; comparing tree IDs propagates read failures.
Stack forks and post-push timing remain unchanged; no deletion targets remain.

Main's #1532 integration changes commit staging and diagnostics, not comparison
ownership. No bounded implementation mismatch was found in this reconciliation.
Release child memory was read in full: its operation-entry lesson supports real
CLI coverage, and its installed v0.13.5 evidence applies only to #1459's older
repair. It establishes no installation of this change.

## Remaining

- PR review: assess the preserved inherited range and short diagnostics. The
  demo's five real-CLI scenarios passed within the affected suites; GitHub was
  simulated, so hosted acceptance remains unproved.
- Hosted CI retains its full matrix. Landing is excluded. Installed Etude
  recovery remains unproved; that checkout was untouched.

No new product decision is required for the implemented approach. Other
source-only refusal paths remain inventory findings, not reproduced repairs.

Checks: network-isolated `cargo test -p loopflow --test task_pr_range_tests --test task_pr_authority_tests --test sync_tests --test pr_tests --test land_tests -- --nocapture` passed all 116 tests; `git diff --check` passed; prior fmt/Clippy/Ruff passes reused for unchanged code; full matrix remains with CI.

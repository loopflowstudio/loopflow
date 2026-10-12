# Stale PR bases

October 11, 2026 · Task lf-cb9012a · PR #1533

Jack Heart requested investigation after `lf pr open` refused in Etude.
This report preserves the investigation, counterevidence and source-level
repair. Disposable CLI tests simulate GitHub; installed Etude recovery and
hosted acceptance remain unproved.

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
| Pruning with an unplaced Task | Earlier gate reproduced four checkout-refresh failures: `protected_worktree_paths` reads the installed registry even with a disposable `LF_HOME`; `read_nonterminal_task_worktrees` decoded nullable `worktree` as `String`. Integrated main now excludes NULL and empty paths. The production read remains read-only; this fix is outside the publication change. |

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

The pre-sync October 11 gate found no further defect in the changed publication code. All
116 affected Rust tests passed under network isolation, including the five-case
real-CLI demo and stack-fork preservation. Formatting, Clippy, Ruff and the
architecture check passed. The broader Python suite passed 409 tests and failed five: the four pruning cases
above, plus a nested macOS sandbox rejected with `Operation not permitted`.
These reproduce the preceding gate's failures; they are not passing checks.
Isolated CI owns the remaining verification. Those results precede current-main
integration and do not establish installed Etude recovery.

After integration of main `83716f9015` into `75371f5ac9`, gate passed all 86
tests in the landing, sync, Task PR authority and Task PR range suites under
network isolation. The five stale-base CLI scenarios preserve every commit;
the squash-parent regression preserves the child's integration fork. Formatting,
Clippy, Ruff and architecture checks pass. No further publication-code defect
was found; review corrected the obsolete pruning finding above.

Python now passes 413 tests, including the four previously failing pruning
cases. Its remaining nested macOS sandbox test cannot apply its policy
(`Operation not permitted`) and remains deferred to capable CI. Headless website
checks pass 74 tests and fail the unchanged homepage's 1,440 px overflow check
(actual width 1,475 px), also previously recorded in Infrastructure memory.
Neither suite is reported as fully passing. The full materialized Rust matrix
remains CI-owned; installed Etude recovery and hosted delivery remain unproved.

The subsequent hosted run [38207213151](https://github.com/loopflowstudio/loopflow/actions/runs/38207213151)
passed 2,426 Rust tests and failed the stale-base CLI demo after successfully
creating its simulated PR. Main's checked review opener exposed a fixture gap:
the test stubbed macOS `open` but invoked Linux's real `xdg-open`, which exited 3
without a browser. The fixture now stubs both platform openers; history and range
assertions remain unchanged. No production error handling was weakened.
Gate synced current main `092ab7ba5b` without conflicts before this repair.

The repaired tree passes all 86 affected landing, sync, PR-authority and PR-range
tests on macOS under network isolation, plus the five-scenario demo, formatting,
Clippy, Ruff and architecture checks. Review found no further production change
needed. Linux execution of the repaired fixture and the full materialized matrix
remain CI-owned; this local result does not establish hosted delivery.

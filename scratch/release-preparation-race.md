# Release preparation raced with incoming migrations

Jack requested this investigation after the minor release stalled. Observed on
2026-10-02: v0.12.30 is tagged, but its candidate cannot be installed because it
contains the unconsumed `remove_ask.sql` migration draft. The installer refused
promotion. Jack's selected CLI remains 0.12.29; the 0.13.0 minor release is unfinished.
Analysis has not repaired the release.

## Incident evidence and causal chain

These findings describe the incident source, before this branch’s repair.

1. **Why was the candidate uninstallable?** Its exact source commit,
   `8cbd0c5b1c99a12151908c59f923b0128706a34f`, contains both the prepared
   `0.12.30.001_release.sql` batch and `migrations/drafts/remove_ask.sql`.
   Preflight on the downloaded candidate refused promotion with “pending draft
   migrations: remove_ask.” Published provenance does not imply complete schema.

2. **Why did a draft survive release preparation?** Preparation and merge operated
   on different trees. Prepared release commit `33b8fb43e` has no SQL drafts.
   PR #1392 introduced `remove_ask.sql`; the final release commit's parent is
   #1392's commit `78b0cc034`. PR #1395 was created at 10:03:46 UTC; GitHub reports
   #1392 merged at 10:13:38 and #1395 at 10:17:31. The ancestry and file contents
   prove the incoming draft was incorporated after preparation. Commit timestamps
   differ from GitHub merge timestamps; neither timestamp alone establishes this.

3. **Why could changed source retain prepared status?**
   `prepare_release_in_worktree` runs the preparation hook before opening and
   enqueuing the release PR. `release_single` then builds the merged commit.
   Its prepared-tree equality check applies to the minor snapshot, not this
   closing patch. The patch path does not establish that the merged tree still
   satisfies migration preparation. Building the exact merged SHA preserves
   provenance, but does not preserve the earlier preparation result.

4. **Why did verification allow tagging?** At v0.12.30,
   `scripts/publish_release.py::_validate_release_candidate` invokes installer
   preflight with `check=False`, reads only `candidate.authority`, and ignores
   both the exit status and rejection verdict. The regression test
   `test_publisher_accepts_published_identity_when_home_preflight_refuses`
   explicitly accepts this behavior. The candidate receipt consequently records
   `installer_verified` despite rejection. Package smoke tests check version,
   help and listing, not installation. Ordinary Rust CI materializes drafts in
   its disposable checkout before testing; the migration namespace checker
   permits valid drafts. Those checks can pass without proving the distributed
   binary embeds a complete schema.

5. **Why does retry remain stuck?** `release_single` resumes an unpublished latest
   tag whenever its hosted build has not failed. It permits replacement after a
   failed build, but a successful build with an invalid install candidate enters
   the resume path. Existing candidate receipts are reused after identity and
   checksum validation. Retrying therefore does not incorporate the missing
   migration or invalidate the old `installer_verified` assertion.

The central ownership gap is between preparing a source tree and accepting its
merged successor. Two downstream assumptions compound it: publication authority
stands in for installer success, and hosted build success stands in for a
recoverable publication candidate.

## Accepted prevention and proof

Jack accepted this repair scope on 2026-10-02. Implementation status and remaining
delivery work are reconciled in [run-records.md](run-records.md).

| Layer | Accepted change | Required proof |
| --- | --- | --- |
| Preparation | Make release preparation converge after integration; any newly incorporated draft requires preparation and a new candidate. Do not rely on an earlier branch's preparation result. | Inject a migration between preparation and merge; the eventual candidate includes it canonically and installs. |
| Exact candidate | Require a complete schema on the actual release source and successful preflight on its packaged binary. Ordinary development draft checks remain useful separately. | Build the incident source unchanged and observe rejection before tagging; a corrected candidate passes. |
| Publisher | Run preflight in a disposable Home and require a successful promotion verdict, rather than ignoring every rejection to tolerate publisher Home state. | Rejected published candidates fail; valid candidates pass independently of the publisher's installed database. |
| Recovery | Provide an explicit path to supersede a tagged but unpublished invalid candidate while preserving tag history. Distinguish invalid artifacts from temporary publishing failures and partial external publication. | Retry this state and reach a corrected patch, then its minor snapshot, without repeatedly selecting the invalid candidate or rewriting the tag. |
| Tests | Extend release coverage through integration, packaged validation and retry. The existing canonicalization test stops before PR integration. | A test that introduces a concurrent draft fails if preparation is only run before integration. |

Implemented locally: exact-source inspection after integration, successor patch
selection for pending migrations, same-cycle minor-pair recovery, fresh-Home
packaged preflight on initial validation and cached reuse, and package-CI
preflight. Controller fixtures cover unprepared integrated sources, immutable
invalid tags, successor selection and partial-publication refusal. Publisher
tests cover exact-commit inspection and rejection despite published authority.
The downloaded incident executable was rejected by the repaired validator.

The controller recovery fixture supplies an already corrected successor; it does
not exercise a real concurrent PR merge through migration canonicalization and
installation. The configured release path remains the outstanding end-to-end
proof. Local implementation and tests do not establish live recovery success.

## Limits and independent failures

- Installer refusal worked. No successful installation of this candidate or
  resulting database corruption was observed.
- The Apple agreement blocker, a transient app capture timeout, and the old
  installed CLI's rejection of `lf repo release publish` are separate failures.
  The CLI mismatch stopped publication after tagging; it did not introduce the
  migration draft. Fixing it alone would leave the candidate invalid.
- The test name suggests publisher Home incompatibility motivated ignoring
  preflight rejection. The original design rationale has not been established;
  the code proves the broad acceptance, not the author's reasoning.
- The original preparation test proved hook execution. Added recovery fixtures
  exercise controller selection; actual concurrent integration and packaged
  installation remain a separate proof boundary.
- Fresh-Home preflight is necessary but does not by itself prove every supported
  existing database can upgrade. Upgrade coverage remains a separate requirement.

## Recovery status and next action

Read-only checks during this investigation found no GitHub Release for v0.12.30.
At that observation, the minor receipt named patch 0.12.30, has no completed patch commit, and
reports `completed: false`. The observed `lf --version` was 0.12.29. No tags or
release receipts were changed during this investigation.

Remaining: land the implemented repair through the existing delivery owner,
then exercise the supported recovery path to produce an installable closing patch
and 0.13.0. Replacement requires fresh absence evidence from GitHub, crates.io
and versioned R2 downloads; the earlier GitHub observation alone is insufficient.
No live release or installation success is established by this reconciliation.
Deleting the tag or manually claiming a completed receipt would erase evidence
without repairing either cause.

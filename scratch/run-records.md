# Exec and Session terminology cleanup

Jack requested a deeper cleanup after `lf doctor` reported a failed “run ledger” audit.

Remove retired Run terminology from active execution diagnostics and trace-journal internals. Doctor identity and scheduled-receipt fixes already belong to PR #1402 in `loopflow.doctor-and-install`; reuse that implementation rather than duplicate it here.

Session captures still live under the published `~/.lf/runs` layout and use retained artifact keys. Preserve that storage contract and describe its current owner; deleting or casually relocating it would break resumable Sessions. Resource accounting must identify these as Session captures.

Doctor/install work is in https://github.com/loopflowstudio/loopflow/pull/1402. Task creation was attempted with both installed 0.12.29 and the verified 0.12.30 candidate; both fail on the foreign-Team Release Stability Project in wave/adoption. No unrelated Project was changed.

The release reached a notarized 0.12.30 candidate. Its publisher requires a newer command tree than the installed CLI; prepending the verified candidate to PATH did not change the publisher’s selected CLI. The publisher change here uses the common `lf release publish` spelling.

Publication remains incomplete. Candidate `8cbd0c5b1c99a12151908c59f923b0128706a34f` is tagged `v0.12.30`, but its merged tree contains `drafts/remove_ask.sql`; promotion correctly refuses it. No GitHub Release exists for that tag. The previously recorded candidate receipt incorrectly accepted published build identity without requiring an installable schema. The new publisher check reproduces the refusal in a fresh disposable Home, and package CI now runs the same preflight before artifacts can be tagged. The installed version remains 0.12.29.

The release runner currently resumes that incomplete tag before considering a newer closing patch. It needs a supported recovery path for an already-tagged, unpublished, un-installable candidate. Do not delete release intermediates or rewrite the tag manually. PR #1402 merged while this investigation was in progress.

Checks: journal tests (16), resource-envelope tests (12), checkout CLI proof (1), cargo build, formatting, and all-target Clippy passed; doctor identity tests passed before duplicate edits were removed in favor of PR #1402.

Publisher tests (6) passed; the actual v0.12.30 executable was rejected by the repaired fresh-Home preflight for its pending remove_ask migration.

## Selected execution: finish releases despite concurrent migration merges

Jack requested launch planning from the [release race investigation](release-preparation-race.md).
Keep the release repair single-threaded in this checkout, alongside the existing
implementation. The core outcome is that `lf release run minor` can recover from
the invalid unpublished 0.12.30 candidate, produce an installable closing patch,
and publish the minor from that patch's exact product snapshot. A concurrent
migration merge must lead to a newly prepared candidate rather than a falsely
verified release. This is the selected implementation scope; details below that
are marked tentative still require code-level resolution.

### Contract and implementation boundary

Jack accepted narrow script contracts with explicit inputs and safe retries.
Preparation owns one source snapshot; validation owns exact artifacts in an
isolated Home; publication reconciles external effects; the controller alone
owns candidate replacement as main changes. Multi-service publication is not
an atomic transaction, so unknown external state must remain an error.

Delete, rather than maintain: acceptance based only on published authority;
cached receipts as a substitute for current artifact validation; unconditional
resumption of a successful build whose source still needs preparation. Preserve
immutable migration history, tags, and valid interrupted publication retries.

- Release preparation applies to the integrated candidate source. Reassess that
  source after merge; if integration introduces drafts, converge through another
  prepared candidate before tagging. Preserve canonical migrations already on
  main and immutable tags. Tentative approach: advance to a fresh patch version
  when another preparation cut is required, using the existing release operation
  and minor-pair state rather than a second orchestration mechanism. Re-running
  canonicalization against an already frozen batch is not an assumed solution.
- Validate the packaged candidate in a disposable Home and require successful
  preflight. Keep the existing local publisher and package-CI changes. A cached
  candidate receipt produced under the old acceptance rule cannot substitute for
  this check when resuming publication.
- Resume transient publication failures using the same valid candidate. When a
  candidate is proven invalid and unpublished, preserve its tag and select a
  corrected closing patch. Update the existing minor pair consistently so retries
  do not return to the superseded patch. Do not treat a missing GitHub Release
  alone as proof that nothing shipped: inspect existing publication stages and
  provider facts before choosing replacement. Uncertain or partial publication
  must remain explicit, never silently reclassified as safe to supersede.
- Keep release-specific decisions in the release operation, schema preparation
  in the migration scripts, and packaged acceptance in publisher/CI. Do not add
  another generic attempt object, planning marker, or manual receipt-edit workflow.

### Done when

1. A test introduces a migration between initial preparation and integration.
   The operation reaches a complete prepared candidate containing that migration;
   the unprepared source never becomes a published release.
2. Published authority plus rejected preflight fails before tagging. Reusing an
   older candidate receipt cannot bypass packaged acceptance.
3. The incident state—tag present, hosted build successful, candidate invalid,
   publication absent—can progress to a corrected patch without rewriting the
   tag. Restarting the operation retains the corrected choice.
4. A valid interrupted publication resumes without duplicate publication;
   uncertain external publication is reported with its unresolved facts.
5. The minor uses the completed corrected patch snapshot. Actual release and
   installation then succeed through the configured path, and installed revision
   and `lf doctor` output are inspected. Any remaining cron failures retain their
   real diagnosis rather than being hidden to declare release success.

Implementation starts with the release operation's candidate selection and minor
pair transition, then closes preparation and cached-validation gaps against that
contract. Focused tests cover the race, rejection, retry, and partial-publication
boundaries. Run required formatting/Clippy and affected tests before checkpointing;
delivery verification owns broader checks. Preserve the incident's useful
contract and evidence in release documentation/PR notes before scratch is cleared.

### Allocation and deferred work

No additional worker or Task is launched: preparation, acceptance and replacement
share the same unsettled release contract and state transitions. Existing local
changes stay with their current checkout. No current Task binding is confirmed;
the earlier Task-filing failure remains unresolved. Task adoption is a tracking
decision, not a prerequisite for this local implementation.

Follow-up **Restore scheduled infrastructure receipts** waits for the corrected
installation; use the resulting doctor evidence to reconcile the actual cron
configuration. Follow-up **File existing Doctor/install work without cross-Team
Project interference** retains the earlier filing blocker and does not reopen the
already merged [Doctor and cron repair · PR #1402](https://github.com/loopflowstudio/loopflow/pull/1402).
Neither follow-up needs a competing release worker now. Broader upgrade-matrix
expansion and signing-environment changes are not selected by this incident.

The selected implementation is now present locally. Source inspection reads the
exact commit, fetching the object when necessary, and reports preparation reasons
separately from publication evidence. Unchecked publication is null, not an empty
list. The controller cuts a successor after concurrent migration integration and
preserves invalid tags. It adopts a completed successor within the minor's patch
cycle after interruption. Cached artifacts receive fresh installer preflight on
both preparation reuse and publication.

Review found two additional assumptions and removed them: GitHub command failure
could masquerade as a missing release, and the minor receipt could still point at
the invalid patch after a corrected patch had published. Service errors now remain
errors; same-cycle successor tags recover the interrupted pair. Existing canonical
migrations and valid publication retries retain their owners.

Remaining: delivery verification and publication of this branch, then the real
closing-patch/minor release and installation. Live replacement must first prove
absence across GitHub, crates.io and versioned R2 downloads; the earlier “no GitHub
Release” observation alone is insufficient. No release tags, provider records, or
installed selection were changed by this implementation. No Flow or worker was
launched.

Checks: release integration suite 62 passed, final source-fetch change's 18 release-run tests passed; publisher 13 and automation 10 passed; cargo fmt, all-target Clippy, Ruff and diff checks passed. Live publication/installation remains delivery work.

Compression removed duplicate Exec terminal classification and repeated release
scope resolution, and finished internal trace/Exec naming without moving Session
captures. The obsolete acceptance paths in the deletion list are gone; no
compatibility implementation remains to maintain. Review retained separate
source-preparation and publication evidence so unknown provider state cannot
become permission to replace a tag.

Checks: `cargo test -p loopflow --lib journal::tests` (16), resource-envelope tests (12), `cargo fmt --all`, all-target Clippy, Ruff and diff checks passed; refreshed-tree gate follows.

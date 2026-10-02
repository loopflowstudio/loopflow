# Install recovery

## Execution decision — 2026-10-02

Keep this change focused on machine installation working from the requesting checkout without resolving Task PR authority. The implementation in `b89725f58` covers download, promotion, rollback and recovery; no further production change is selected. Continue this branch through its existing review and delivery path. This planning step does not publish, install, or start another worker.

The recorded focused checks establish the early authority boundary, not successful recovery of the affected installation. Retain the candidate-identity uncertainty and outstanding published replay as acceptance limits during review.

Deferred outcomes:

- **Prove upgrades and interrupted recovery from a checkout.** Repair the disposable release fixture and exercise an initialized older installation against a published candidate containing this fix, using the acceptance conditions below. This remains proposed scope; no Wave or Task owner was supplied. File or reuse work once ownership and scope are selected, with the design and evidence transferred before launch. The affected Mac replay additionally depends on a released fix.
- **Explain missing scheduled receipts.** Investigate the two exact Doctor cron-history results separately. The supplied evidence does not establish a scheduler defect or select a repair; preserve that uncertainty rather than expanding this installation change.

No Tasks or workers were launched. Review decision: do not turn the unverified replay fixture findings into an implicit implementation commitment or claim that the incident is resolved.

Jack Heart reported `lf install` failing after download with “Task PR authority refused” and an installation/store mismatch. The subsequent `lf doctor` ran the old release, reporting missing scheduled receipts and a stale binary.

The candidate reaches `install promote` in the original Git checkout. Its checkout guard resolves Task PR authority through an ordinary store open before installation compatibility checks. That store open can reject the candidate precisely because the installed runtime/schema is different. The same dependency exists in switch recovery.

Repair: remove Task checkout gating from machine installation, including promotion, rollback and recovery. Retain the existing published-artifact, migration, lifecycle, switch ownership and promotion-lock checks. Installation can run from the checkout where it was requested.

Checks: `cargo test -p loopflow --test global_commands installation_` (2 passed), `cargo test -p loopflow --test local_promotion` (2 passed), `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `uv run --project website --extra test pytest website/tests/test_readme_index_sync.py`, and `git diff --check` passed. Live published installation still needs replay with a released fix; a source build cannot be promoted. Missing cron receipts are independent and are not repaired by changing installation authority.

Review finding resolved: removing only the download-path guard would leave promotion and interrupted-switch recovery with the same dependency. Removed all installation callers and the now-unused Task helper; retained candidate and switch authority checks. No schema changes or production-data edits.

## Incident analysis — 2026-10-02

Recovery status: the repair is committed locally in `b89725f58`. Jack Heart's supplied transcript still ends on installed revision `f4cf378af`; no successful published upgrade is evidenced. This investigation changed only this note and did not install a candidate or modify production data.

### Causal chain

1. **The download completed, but activation failed.** The transcript records a valid DMG checksum followed by Task PR authority refusal. `release/install.sh:185` invokes the downloaded CLI's `install promote`; neither that invocation nor `published::latest` changes the caller's working directory. The candidate therefore encounters the original Git checkout.
2. **Installation depended on Task registry access.** Before the repair, `guard_task_checkout` ran for any discovered Git repository or explicit Work declaration. It called `require_unmanaged_checkout`, which called `resolve_managed_task` and `open_registry_for_authority`. The latter uses ordinary `open_store`, and Task resolution wraps an opening failure as “Task PR authority refused.” This happens before Task ownership can be resolved; the failure does not prove that the checkout belonged to a registered Task.
3. **The ordinary-runtime prerequisite preceded the installation boundary that handles compatibility.** The old `promote` called this guard before entering promotion. The current `promote_published_from_machine_install` owns the exclusive lock, validates retained artifacts, reads candidate preflight, requires published authority and rejects incompatible migration/lifecycle evidence. An ordinary store open can reject a candidate before those checks can decide whether it may replace the installation. `store::FrontierAdvance` explicitly reserves shared-store migration for promotion. Making installation depend on ordinary runtime compatibility defeats that boundary.
4. **A Task execution restriction became a checkout restriction.** Commit `d296b4805` replaced `guard_task_origin` with `guard_task_checkout`, extending the check to discovered repositories. This explains why an interactive installation from a checkout could enter PR-authority resolution. The previous `installation_restriction_uses_checkout_or_explicit_declaration` test expected rejection for a managed checkout and explicit Task declaration. It enforced the restrictive policy rather than proving that installation could bridge an existing runtime to a new candidate.

The supported design cause is the dependency of machine installation on unrelated Task authority. Removing that dependency is already implemented; weakening ordinary store isolation or adding more permitted checkout cases is unnecessary.

### Related symptoms and limits

- `journal::run` prints “Exec history unavailable” when no compatible process context exists, then returns the command's result. That warning is not itself the installation refusal.
- Doctor ran the old release and reported its own known migration matching the applied frontier. Selection, store and fallback checks passed. Its fatal result came from two missing current scheduled receipts; binary freshness was a warning. Historical ledger gaps were additional context. These observations do not establish store corruption or explain why the scheduled receipts were missing.
- The exact inner error “belongs to another installation” is absent from this checkout and its pre-repair parent. The transcript does not identify the downloaded candidate's revision. The outer Task-error path is supported by source and the transcript; the precise candidate-side identity check remains unverified. Do not substitute a proven schema migration failure for the observed installation-identity refusal.
- The two `global_commands` regressions establish that checkout/declaration context and an unreadable registry reach the candidate-authority verdict. They intentionally use a non-published build and stop at refusal. `local_promotion` covers preview shape and preservation of store contents. Neither proves a successful published upgrade, rollback, or interrupted-switch recovery from the affected checkout.

### Prevention and next evidence

Already implemented: keep installation independent of Task checkout authority across download, promotion, rollback and recovery, while retaining installation's own checks. The recorded focused tests protect the early boundary. No additional production abstraction or prompt rule is warranted by this evidence.

Proposed follow-up, not an approved expansion of this repair: restore and extend the disposable release replay fixture before relying on it as upgrade evidence. `tests/e2e/install_bootstrap.py:48` passes `--mode interactive` to `env`, and line 87 references undefined `DAEMON` although the archive contains only `lf`. These are source-inspection findings, not a claimed container run. Its optional prior-release case copies binaries without initializing the prior release's store/installation, runs outside the checkout, and disables Git before installation cases. It therefore does not exercise the observed combination of a discoverable checkout, initialized older installation and new published candidate.

The useful next acceptance check is an isolated old-to-new published installation from a discoverable Task checkout: establish the old selection/store first, run the verified installer, and assert the new CLI selection, recognized store frontier and unchanged checkout contents. Exercise interrupted-switch recovery from that checkout as well. Use a disposable container/account; overriding `HOME` alone does not isolate machine installation. A released-fix replay on the affected Mac remains the final recovery evidence and must record both candidate and installed revisions.

Investigate the missing scheduled receipts separately using the two exact cron-history commands printed by Doctor. Missing receipts alone do not distinguish scheduler inactivity, failed execution or missing recording; this installation patch cannot establish their cause.

Review finding: distinguish reaching candidate refusal from completing an upgrade, and preserve the unresolved candidate identity instead of claiming the precise inner failure was reproduced. These limits are now explicit above.

Analysis check: `git diff --check` passed; prose-only investigation reused the recorded implementation checks without rerunning builds. Release/container replay remains outstanding.


## Doctor architecture repair — 2026-10-02

Jack Heart requested Doctor diagnosis against the current architecture and a landed PR.
The installed 61d21f885 replay reproduces two missing cron receipts and one invalid
repository value. Exec now includes machine commands with absent repository scope;
that absence is valid. Relative recorded repository paths remain invalid.

The two exact cron histories have no receipts in the past two days. Both launchd
specs pin lf-d7bf7c66843517e437c870e2dea0beb52717f4633f73441eedba01ff5e7bc401.
Their logs report inactive retained artifact refusal, then missing Daemon-role
failures from old startup code. These are startup failures before receipt recording,
not evidence of corrupt Exec history. New cron installation uses the stable machine
entry gate; Doctor retains missing-receipt failure and names executable, log and
reconciliation commands, including repository-only schedules. Freshness output is
bounded and distinguishes merged source from published releases.

Live scheduler recovery still requires the published repair, cron reconciliation,
and a real scheduled receipt. Do not trigger release-run merely to make Doctor green.
The earlier installation acceptance limits remain in force.


Replay evidence: copying the full 0.12.29 database into a disposable Home is rejected
at the 0.12.30 canonical frontier, as required by its isolation contract. Importing
all 3,990 unchanged Exec rows into a fresh fixture and copying the installed cron
specs reproduces only the two real missing receipts: identity passes with three
absolute repositories and 71 repository-free Execs; attribution and lineage pass.
This is history diagnosis evidence, not a published upgrade or live scheduler replay.

Review finding resolved: repository Task schedules have no Wave; their diagnostic
must use `cron sync --repo` and quote the empty Wave for history. Preserve missing
receipts as failures rather than laundering startup failures into a healthy ledger.
The entry-gate test uses disposable true/false executables and proves job-path
continuity, not successful production publication or release execution.

Checks: `cargo test -p loopflow --lib doctor::tests` (10), `cargo test -p loopflow --lib ops::cron::tests` (16), `cargo test -p loopflow --test doctor_tests` (3), installation regressions (2), local promotion (2), `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, README/index sync (1), and `git diff --check` passed.

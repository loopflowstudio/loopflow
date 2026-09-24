# Publisher checkout slice review

## Verdict

**Iterate. Keep the serial PR unpublished and the Task incomplete.** Inherited
checkout protection advances the complete design. This review reproduced and
repaired a cleanup bypass that the controller-death proof did not cover. The
remaining execution and acceptance gaps still belong in this Task.

Starting head: `713326c4ba0291184399dd2f717569db41ad7e34`. Obtained the complete
534,917-character Task patch with `lf task diff LOO-285 --json`
(`truncated: false`). Reviewed the directive, full design and forbidden outcomes,
current slice, previous evidence, lease/target owners, their child launches and
cleanup consumers, Python forwarding, CLI attribution and settlement/history.

## Demonstration and repair

The existing built-CLI demonstration passed preparation and publication after
the exact fixture controller was killed and reaped (8.06s). Ordinary removal
failed while the publisher survived, another checkout remained removable,
same-target release deferred, and cleanup became available after child exit.
Disposable repositories use real Git, processes and OS locks; hosted workflow
and publication are simulated. This does not establish live signing, UI-host
verification, launchd timing or either configured settlement.

Source review found that controller-owned cleanup used `worktree_remove_owned`
with its existing lease. That capability can be shared with a surviving
descendant. When a publisher launcher exits, the controller can therefore
delete the checkout despite the descendant still holding that same lock.
Inheritance protects ordinary removal but cannot make this bypass safe.

Extended the CLI fixture with preparation and publication launchers that leave
a blocked descendant, redirect its output, and exit unsuccessfully. The
controller reaches its real cleanup path without being killed. Before repair,
the preparation case failed with `controller cleanup removed surviving prepare
descendant's checkout`.

`cleanup_release_worktree` now consumes and drops the controller's lease before
calling ordinary removal, which acquires an independent open file description.
A surviving descendant keeps that acquisition blocked. Cleanup reports the
retained checkout and returns before branch deletion. When no descendant holds
the lease, ordinary cleanup proceeds. All four call sites transfer ownership;
no ambient descriptor bypass, new lock, liveness record or cleanup service was
added. Owned removal during exact-source materialization remains separate;
this change covers cleanup after child execution.

The expanded proof passed all four stage/exit combinations (14.68s), including
retained source bytes, unrelated checkout removal, target exclusion and cleanup
after descendant exit. The six Python helper/authority cases also passed:
both descriptors survive helper/uv exit, while checkout-only possession cannot
enter publisher verification or reconciliation. Python behavior was unchanged.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| Publisher survives controller death | Checkout and target remain protected | Both existing descriptors reach prepare/publish/reconcile | Built-CLI prepare/publish kill cases; reconciliation launch inspection | pass locally; reconciliation kill point not exercised |
| Descendant survives launcher failure | Controller cleanup cannot delete active source | Cleanup drops its handle and reacquires through ordinary removal | New regression failed before repair; four-case proof passed after | pass locally |
| Distinct checkout authority | Other checkouts remain independent; checkout fd cannot authorize publication | Exact-path worktree lease remains separate from target lock | CLI unrelated removal; Python checkout-only rejection | pass locally |
| Descendants retain ownership | Python process boundaries preserve both lock lifetimes | Publisher, website and packaging helpers pass both fds | Six focused Python cases | pass locally |
| Complete mutation exclusion | All mutation children retain exclusion | Shared PR/commit/re-arm, hooks, notes, materialization and cleanup subprocess paths remain uncovered | `prepare_release_in_worktree`, `wait_for_pr_merge`, hook and Git command construction | gap |
| Original dues, repeated wakes and frozen catch-up | Stable keys; one execution/settlement; later dues wait | Atomic obligation document and frozen covered keys retained | Accounting source and earlier focused receipts | retained local evidence; not rerun |
| Candidate retry and late evidence | Same candidate owner; exact attempt fence; no terminal regression | Selection survives preflight; `settle` writes outcome/proof atomically | Selection/settlement source and earlier regression receipts | retained local evidence; full interruption matrix gap |
| Process exit versus product result | Zero exit cannot establish publication | `finish_process` leaves missing product proof Unverified | Writer inspection and prior joined/shell cases | retained local evidence |
| Manual provenance and timing | Collapse retains intervention and first attempt; no false unattended pair | History consumes collapsed provenance and frozen coverage | History source and prior regressions | retained local evidence; live trigger race gap |
| Required telemetry | Every original due has prerequisite association and bounded current recovery | Only current interval and two-day receipt lookup; no prerequisite retry | `verify_scheduled_telemetry`; retained 36 failed targets | gap; repair ownership unassigned |
| Exact candidate/no-change checks | Immutable subject checked; verified baseline and empty range | Shared candidate completion and public proof gate remain | Source and prior wrong-source/joined cases | retained local evidence; live proof gap |
| Complete public reconstruction | All stages, hashes, UI and exact-version smoke required | Publisher observes/repairs supported stages and rejects missing proof | Publisher source and prior recovery cases | retained local evidence; actual public/UI acceptance gap |
| Post-publication failure | External effects survive without false success | Stage receipts retained before later smoke/settlement | Prior joined and Python failing-installer cases | retained local evidence |
| Reconfiguration and closed work | Retain denominator and actionable continuation without Home transfer | Old segments retained; unfinished attempts lack disposition/continuation | `observe`, `close`, `receipt_context` | gap |
| Corruption, DST and unknown history | Fail explicitly; preserve historical uncertainty | Strict records and shared calendar remain | Source and earlier focused receipts | retained local evidence |
| Caller preservation and isolation | Caller bytes survive every exit; independent scopes progress | Earlier joined five outcomes preserve bytes; current proof preserves active source and unrelated cleanup | Prior joined receipts plus current CLI cases | partial; remaining interruption cases open |
| Two adjacent automatic settlements | Distinct automatic executions, at least one real publication, all verification, no manual repair | No qualifying configured pair demonstrated | Retained Home history and acceptance ledger | gap |

## Negative architecture and next direction

The repository release flow remains one mechanical operation. Explicit cron
receipt/job ownership supplies attribution; the target lock excludes release
mutation; the checkout lease protects removal. `settle` remains the sole typed
product writer, and `finish_process` cannot manufacture product success.
Searches found no release-selection `sync_main`, duplicate success-proof types,
`record_verification` writer or separate Python candidate/publish receipt
classes. Historical schema-1 receipts remain intact.

Next, carry the existing exclusion through shared commit/PR/re-arm operations,
hooks, notes and source/cleanup mutation children, with surviving-child proof.
Keep the independent target and checkout scopes. Complete historical telemetry
associations, bounded prerequisite recovery and dated ownership; resolve closed
unfinished obligations without transferring Home authority; finish interruption
proof before supported install/sync and configured acceptance. The observed
missing `agent_turns` scorecard table remains a verification blocker and the
Intelligence handoff remains unaccepted. No independent new live publication
blocker was demonstrated or sibling Task opened.

## Validation

- Original controller-death CLI demonstration: passed, two stages, 8.06s.
- `cargo test -p loopflow --test release_lock_tests surviving_publisher_keeps_its_checkout_after_controller_exit -- --nocapture`: failed before repair; passed afterward, four scenarios, 14.68s.
- `uv run pytest python/tests/test_release_publisher.py -q -k 'descendant_retains or direct_publisher_stage'`: six passed, 17 deselected, 0.38s.
- `cargo test -p loopflow --test release_tests release_run_prepares_signed_artifacts_before_pushing_the_version_tag -- --nocapture`: passed, 12.73s.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`: passed.

No full affected-suite gate, hosted matrix, UI automation, installation, cron
trigger, production publication, PM handoff, PR publication, landing or Task
completion was performed.

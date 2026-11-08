# Server request ID reuse incident

2026-11-08. GitHub main CI run [42457183906](https://github.com/loopflowstudio/loopflow/actions/runs/42457183906), push `1c757e9fe2028c0073414dc6826395931a5aa743` after PR #1813, failed `rust-test (library, 4, 5)`: 489 passed, one failed. Saved log: `/tmp/loo593-main-failure.log`.

At 07:10:33 UTC, `server::service::tests::hello_negotiates_and_requests_answer_by_id_in_either_order` failed at `service.rs:1041`: after responses for `a` and `b`, sending another request with ID `a` did not produce the expected Response. The log does not print the received message.

The unbreak step was scoped by the incident coordinator to reproduce and repair locally before returning for cause review. This was a step handoff, not a quotation from Jack Heart or a restriction on later incident delivery.

The checkout was clean at `b7d41a517`; its `service.rs` is byte-identical to the failing main SHA's file. No data migration or provider restart is needed. Follow-ups 807, 803, 808 and mobile PR #1812 are separate work; leave their checkouts and processes alone.

Working means a client can reuse an ID immediately after its final response, while an unanswered request or live subscription still reserves its ID. Cancellation must not let an old worker's response affect a newer request with the same ID.

Inspection of the original code: `request` queues its response before returning its ID. The session selects socket delivery and `join_next_with_id` independently; only joining removes `inflight`. Delivery can therefore precede retirement, falsely rejecting immediate reuse. Joining can also precede delivery, releasing the ID too early.

## Local checks

- Original failing test: passed once on this Mac against the unfixed source.
- An initial 256-round immediate-reuse socket stress test passed 100 unfixed runs; `/tmp/loo593-request-id-reproduction.log` retains those observations. Replaced it with a synchronized regression rather than relying on this Machine's scheduling.
- The synchronized regression holds the first worker after it queues its response, before it returns to the JoinSet; the client receives that response and immediately submits another request with the same ID. A test-only one-shot gate forces the suspected ordering, without sleeping or retrying a failed request.
- `cargo test -p loopflow --lib server::service::tests::answered_request_id_can_be_reused_before_its_worker_returns -- --exact` with inherited `LF_*` removed and stdin closed: failed against HEAD plus the regression and gate, returning `Error { id: Some("held-completion"), code: Conflict, message: "request ID is still unanswered", ... }` after the first response was received. This confirms the hypothesized order causes the real refusal.

## Repair

Final response delivery now releases the ID in the session loop before writing to the socket. Joining workers only reaps them; it no longer controls admission. Queued messages carry their Tokio task ID so an old cancelled worker cannot answer or retire a newer request with the same wire ID. Subscription acknowledgements and readings keep the ID reserved; errors and `ended` release it. The response queue stays bounded; already-admitted mutations and existing execution owners are unchanged.

- `cargo test -p loopflow --lib server::service::tests::` with inherited `LF_*` removed and stdin closed: all 14 tests passed, including the synchronized regression, original CI failure, subscription ID reservation, cancellation and reuse. Baseline and repaired test builds both emitted macOS ld's existing `__eh_frame section too large` warning; execution succeeded after repair.
- `cargo fmt`: passed.
- `cargo build -p loopflow --bin lf` with inherited `LF_*` removed and stdin closed: passed.
- `uv run python /tmp/loo593-request-id-probe.py`: passed against the newly built `lf server run`, with a temporary HOME/LF_HOME, empty working directory and private Unix socket. The real protocol completed 512 rounds of concurrent `a`/`b` requests (1,024 responses), 128 error-then-success reuse pairs, reuse after a subscription error, live-subscription conflict, and immediate reuse after cancellation. No request sleeps or retries. The probe stopped its server and removed its home/socket; it did not register or replace the installed service.
- `cargo clippy --all-targets -- -D warnings` with inherited `LF_*` removed and stdin closed: passed.
- `git diff --check`: passed.

The local request-ID workflow is restored. No publication or landing in this step. Main CI and installed binaries have not received this repair; Linux CI verification remains for the later delivery step. Follow-up checkouts and their running work were not changed. The pre-existing `scratch/server-request-id-ci-observation.md` remains untouched.

## Debug review — 2026-11-08

Reviewed the repair, request/subscription producers, cancellation/checkpoint paths,
and the saved socket probe. No further defect was substantiated; production code,
tests and server documentation remain exactly as checked by unbreak. Reused its
14 service tests, isolated socket probe, build, fmt and clippy results without
rerunning them.

The synchronized failure establishes a real ordering bug, not the exact message
received in CI: that assertion omitted the received variant. The original code
also allowed joining before delivery, so merely prioritizing joins would trade
late retirement for early retirement. Final delivery is the event both sides can
use; worker completion is not.

Neighboring paths reviewed:

- Cancellation removes the entry before aborting its worker. Aborting cannot
  retract an already queued message; the task-ID check discards it whether the
  wire ID is absent or belongs to a replacement. Late worker joins now change no
  admission state. No provider or admitted blocking write is restarted or replayed.
- Subscription acknowledgement and readings retain their checkpoint sender and
  ID. Final error or `ended` removes them. A duplicate-request Conflict is sent
  directly and leaves the original subscription live; the existing regression
  checks a subsequent checkpoint reading after that Conflict.
- Checkpoint overflow removes/aborts the subscription before sending Busy, so
  its queued readings are subsequently discarded. A checkpoint racing a queued
  terminal message can instead receive Busy because the worker closed its inbox;
  either path ends that subscription once and permits reuse.
- Retirement happens before the awaited socket send, but the session cannot
  admit another incoming request during that send. Send failure exits the session
  and drops its remaining workers; no reusable half-open connection remains.

Coverage limit for later review: the saved cancellation checks do not force an
old message to remain queued across cancel/reuse, nor do they force checkpoint
overflow. Those paths were inspected, not newly exercised. No Linux run or
installed-service replacement was performed in debug. Return to 5whys with the
repair unchanged; nothing was published.

## Cause review — 2026-11-08

Jack Heart's report was a failure screenshot and `$incident`; the step handoffs
are coordinator direction, not human quotations. Observed impact was red main
CI and skipped main packaging. Clients reusing an answered ID could receive a
false Conflict; no affected person's live request or provider loss was observed.
The repair is locally verified and uncommitted, not shipped. This analysis
changed only this note and reused the checks above.

### Production cause

The session used worker reaping as the end of an unanswered request, although
the client sees response delivery. Those events ran in separate select branches.
The forced ordering—queue response, hold worker, deliver response, reuse ID—
returned the recorded Conflict on the old code. Worker lifetime had therefore
been substituted for protocol lifetime. Releasing on final delivery removes
that dependency; prioritizing joins would still permit retirement before an
answer was delivered. The task identity on queued messages also keeps a cancelled
worker's answer from reaching a replacement request with the same wire ID.

This establishes the production defect. The missing received variant in the CI
assertion prevents claiming the exact CI interleaving was observed. No further
cause for the original implementation choice is established.

### Scheduling coverage

The original test already asserted reuse after receiving both `a` and `b`.
It did not hold the first worker or control when the session reaped it; waiting
for the sibling response also gave reaping another opportunity. Thus it exercised
the behavior without reliably exercising the vulnerable ordering. One ordinary
local pass and 100 unfixed stress passes did not disprove the race. The one-shot
regression now requires reuse to succeed while the old worker cannot finish.
Its old-code failure and repaired-code pass are the useful prevention check;
more retries or a longer timeout would not supply that coverage.

Read-only GitHub checks during this step reconfirmed the observation note:
merge-group run 42456217130 succeeded, including library shard 4 (completed
06:57:30 UTC); push run 42457183906 failed on the identical merge SHA; package
run 42458161758 was skipped. The PR run also succeeded. This contradicts a
simple explanation that the queue tested a different commit or omitted this
shard. It does not establish a Linux-only defect or identify a particular
runner-load trigger. Cancellation with a deliberately queued old answer and
checkpoint overflow remain inspection-only limits, as recorded above.

### Post-merge observation

The landing driver returned at 07:03:14 UTC; the failing assertion occurred at
07:10:33 UTC. That return could not certify the later push result. The subsequent
report, “#1813 merged last night with all checks passing.”, inspected PR checks
only (see [CI observation](server-request-id-ci-observation.md)). It generalized
a PR result to a claim that also implied main was green, leaving the push failure
unreported in that update. It did not cause the race or invalidate the earlier
successful queue run.

Reviewed both installed and repository `verify` and `pr-land` skills,
`task-operate` completion guidance, and `.github/CI.md`. Landing is documented to
stop at merge; verification already requires checking main and recording what
was checked. CI documentation distinguishes PR, merge-group and push runs.
The verification skill does not specifically direct inspection of the merge
SHA's push run. Nothing here explains why the later operator selected only PR
checks, and there is no basis for a broader monitoring redesign.

### Prevention still proposed

- Add one instruction to the existing `verify` skill: inspect post-merge CI for
  the exact merge SHA and report its event, run link and result separately from
  PR/queue checks; say pending or unobserved when that is all that is known.
  Check the wording against this incident's green queue/red push pair: it must
  produce a red-main finding despite successful PR checks. Keep landing's
  existing stopping point. This guidance change has not been made.
- Include the received message in the original reuse assertion's failure text,
  as the new regression already does. This would resolve the diagnostic gap in
  a future failure, without changing scheduling or masking a failure. Not yet
  changed; the current CI message cannot be recovered retroactively.

The production repair, synchronized regression and protocol documentation are
already local changes; neither proposal above is shipped or approved by this
analysis. Next: return this note to launch-plan to scope delivery of the verified
repair and decide the small guidance/diagnostic changes, retaining Linux CI and
post-merge verification as outstanding checks. No publication, new Task or
follow-up Graph was started here.

## Launch decision — 2026-11-08

Keep the delivery-retirement repair, synchronized regression and protocol docs
in LOO-593's current checkout. Include both small prevention changes here:
the original reuse assertion now prints the received message, and the built-in
`verify` skill requires the exact merge SHA's push result separately from PR and
queue results. No new Task, Session, Graph or monitoring project is needed.
The parent conversation owns publication, landing and post-merge verification.

Read-only Task/PR inspection found these overlaps:

- [#1855 · Operation names](https://github.com/loopflowstudio/loopflow/pull/1855),
  LOO-807: shipping, CI running. `service.rs` changes enum names and the README
  documents naming. Keep those names when syncing; no behavioral choice is needed.
- [#1854 · Recover submitted creations](https://github.com/loopflowstudio/loopflow/pull/1854),
  LOO-810: its implement step is running. The outbox patch already retires IDs
  on delivery and filters frames by worker identity. It changes the same session
  loop and producers. Deliver this narrow repair independently; when combining
  them, keep outbox reservations/budgets, delivery-based retirement, stale-worker
  filtering and the synchronized regression. Do not restore join-based retirement
  or add a second queue. No decision from a person is required for these invariants.
- [#1812 · Native iOS](https://github.com/loopflowstudio/loopflow/pull/1812),
  LOO-718: published at demo, settlement unarmed. Its diff still includes the
  server file and README from before #1813 landed. Sync against repaired main
  before its delivery and retain this regression with remote admission intact.
  Its separate mobile product choices stay there.

LOO-803/#1852 and LOO-808/#1853 are shipping with CI running and have no overlap
with these three source files. Leave all existing follow-up execution alone.
The queued-cancellation and checkpoint-overflow coverage limits above remain
limits, not new incident Tasks or reasons to repeat diagnosis.

### Parent delivery/check plan

1. Keep the new PR scoped to immediate answered-ID reuse, subscription reservation,
   stale cancelled-worker suppression, and the two prevention edits. Before scratch
   cleanup, put the forced old-code Conflict, repaired checks, inspection-only limits,
   overlapping PRs and remaining Linux/push checks into the PR body. The incident
   notes and `/tmp` probe/log are local, not permanent published artifacts.
2. `lf sync` in this checkout, then inspect the diff against refreshed `origin/main`.
   `scripts/test.py --list` before sync selected 480 historical paths; reclassify
   after sync instead of running that stale full plan. Keep #1813 as delivery
   history; the incident needs a new PR in the same Task.
3. On the final candidate, run `cargo fmt`, the 14 service tests (including the
   synchronized regression and original failure), and
   `cargo clippy --all-targets -- -D warnings`, with inherited `LF_*` unset and
   stdin closed for Rust checks. Use the affected gate plan after sync, reusing
   only matching content/command results. Rebuild `lf` and rerun the isolated
   socket probe if sync changes production behavior; no installed-service change
   or provider restart is required. Linux execution remains CI's check.
4. Use `lf commit` and `lf pr publish` for the new repair PR; inspect its title,
   body and actual base diff. Then the parent uses `lf land --wait-and-fix` for
   scratch cleanup, required PR/merge-queue checks and verified merge. Do not
   call landing against the old merged #1813 and mistake that no-op for delivery.
5. Read the repair PR's `mergeCommit.oid` from GitHub. Query
   `gh run list --workflow ci.yml --event push --commit <merge-sha> --json databaseId,headSha,event,status,conclusion,url`.
   Inspect that exact run with `gh run view <run-id> --json headSha,event,status,conclusion,url,jobs`.
   Wait for its result; require successful main CI, including `tests-result` and
   library coverage. If jobs legitimately reused the matching merge-group run,
   inspect that linked run. PR or queue success alone is insufficient. A missing
   or unfinished push run remains unobserved or pending; a failed run needs a
   concrete repair, not retries to hide this race.
6. Check the corresponding `Packages (main)` workflow_run triggered by that
   successful push; verify the packaged source SHA and result, separately from
   CI. Report a skip/failure/pending build as such. Do not claim an installed
   binary was updated. Record the repair PR, merge SHA and event/run/result links
   before calling the incident verified or completing LOO-593.

The new verify wording was checked against live GitHub results: merge-group
42456217130 is green and push 42457183906 is red for the identical
`1c757e9fe2028c0073414dc6826395931a5aa743`. It therefore reports failed main CI,
without changing when landing returns. No publication or landing occurred here.

- `cargo test -p loopflow --lib server::service::tests::` with inherited `LF_*`
  unset and stdin closed: 14 passed after the assertion/verify edits; the existing
  macOS linker unwind warning remained. Parent reruns on a changed candidate.
- `cargo fmt` and `git diff --check`: passed after these edits. Parent owns final
  candidate clippy/gate, Linux CI, exact merge-SHA push and packaging checks.

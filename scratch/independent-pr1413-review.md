# Independent review of PR #1413

Reviewed for Jack Heart on 2026-10-04. Recommendation: request changes for finding 1.

Scope: published head `71605f3832136867b86909d5f064b2d67aa3b778`, base
`4cb891ee8d1f31ca99b029a55806df252e24eb2a`. Read the actual GitHub diff,
implementation/callers, and the design, decisions and review retained at
`38b930288:scratch/`. Local HEAD `ae308cc06` also contains #1422 and memory
curation; those additional changes are not attributed to this PR.

## Findings

1. **P2 — correct the false negative publication claim before merging.**
   `rust/loopflow/src/ops/task.rs:5184–5186` wraps every steer error with
   “Restart advice was not published.” But `ops/linear_observe.rs:55–58`
   explicitly returns an error after a successful comment creation when the
   subsequent observation/local reconciliation fails. Its other failure path
   (`linear_observe.rs:113–119`) preserves uncertainty when both the write
   response and marker readback fail. Both become a categorical denial here.
   A dropped connection can therefore tell Jack to treat already-published
   advice as absent; resubmission creates a fresh marker and can duplicate
   direction, while the preserved worker can still reconcile the original.
   Keep the underlying confirmed/unknown result and say only that restart did
   not replace the Flow/worker. Cover successful creation followed by failed
   observation and uncertain write/readback. The existing offline-advice case
   at `tests/task_restart_tests.rs:182` instead enshrines the false claim.

2. **P2 — nonblocking diagnostic regression: retain a safe failure category.**
   `rust/loopflow/src/ops/read_retry.rs:21–31` replaces every permanent error
   with “read failed after 1 attempt(s)” and logs the same generic category.
   Missing `gh`, disk/permission failures creating the download directory,
   HTTP 401/403, and missing artifacts are now indistinguishable. These require
   different recovery actions, and the prior check-reader error exposed the
   cause. Retain safe structured categories/status codes and local I/O kinds
   without emitting raw stderr, signed URLs or credentials. This does not
   cause false success or duplicate publication.

## Correctness and evidence limits

- Retry closures contain only artifact downloads and individual check-page
  reads. Each download gets a fresh TempDir, retaining the selected workflow
  ID; publisher preparation and tagging remain outside retries. Exhaustion
  propagates an error. Every check page still validates head and commit before
  contributing gate evidence; missing contexts remain unknown.
- Restart, continuation and managed admission use the shared cached reader.
  Available-state, terminal-state and ownership checks remain, and acquisition
  timestamps are preserved. Accepting old valid observations matches Jack's
  recorded approval. No new execution/claim authority bypass was identified.
- The restart fixture proves a replacement mechanical step reaches review
  and preserves Task/PR identity and the old Flow row. Its original Flow has
  no live worker/claim; it does not independently prove fencing an active
  provider, preserving all old history contents, or invalidation arriving
  between replacement steps. These are coverage gaps, not demonstrated bugs.
- GitHub CI run [37186227700](https://github.com/loopflowstudio/loopflow/actions/runs/37186227700)
  passed for the exact published head, including Swift and network isolation.
  The PR body's statement that fresh CI is still required is stale. Existing
  Swift shipment evidence is retained; no new live release or installed
  acceptance is established by these fixtures.

Verification: source/call-path and retained-test review plus exact-head hosted
CI readback; no local suites rerun. Only this review artifact was written.
No code, commits, publication or merge state changed.

## Repair outcome — October 4

Jack Heart authorized holding, fixing and landing. Finding 1 is repaired: the
restart wrapper reports only that replacement stopped, retaining the underlying
confirmed publication or uncertain-write guidance. Finding 2 remains nonblocking.

Check: confirmed/uncertain steer regression, task_restart_tests, cargo fmt and cargo clippy --all-targets -- -D warnings PASS.

# LOO-377: retire the exact review before restart

Jack Heart authorized implementation and verified landing on October 4 without
interactive gates. The incident evidence is preserved separately in this scratch
directory and Git. LOO-370 and LOO-373 remain untouched.

Restart must retire and fence the pending review, then stop only execution named
by its captured input, current driver/provider and exact process receipts. The
Session closure records administrative retirement, never successful feedback.
Retain those identities until stop can be retried; replace the Flow only after
all exact owned execution has exited. Independent reviews retain their blockers.
The existing Session dispatch lock and generation fence serialize retirement with
native writes. No new lifecycle object or successful completion is introduced.

Delete — do not maintain: the assumption that stopping only the Flow claim and
selected step establishes review quiescence. Retain the existing stop path for
ordinary steps and keep history, captures, native IDs and Task/PR/checkout identity.

Implemented: exact review closure and driver fencing; explicit service Exec
association; persisted PID/start observations before native receipt cleanup;
bounded exact-process stop; independent-work admission check; replacement requires
stop evidence. Captured input and native history remain unchanged, with no
successful review event. No schema migration or process-group inference is needed.

Compression/review found a missing independent-review admission check in restart;
it now shares continuation's blocker reader. The service registration lives in the
shared review launch path, including opening locally and the background service.
Retirement/replacement rollback is exercised directly; public restart uses actual
process interruption because custom SQL triggers invalidate disposable Home schemas.

The launch lock now precedes the driver fence, matching review opening. A live
launch-lock fixture adds an exact child and advances the saved version while
restart waits; the child is then stopped and the fresh same-Flow position is used.
The lock has a bounded wait. A failed replacement launch resumes the same saved
replacement identity through public Flow start.

Remaining: hosted CI and verified merge. The changed-aware gate stopped at its
resource preflight (13.3 GiB free below the repository's 32 GiB reserve); product
suites and Clippy are deferred to hosted CI. No inactive build was eligible, and
uv cache pruning returned busy. No active checkout or installed Home was changed. Installed acceptance remains separate. The idle managed Flow
must remain truthful during supported delivery; no synthetic step completion.

Check: focused public restart and store retirement tests, architecture and fmt
passed; `uv run python scripts/test.py --reuse-passing` failed resource preflight,
with Rust/Clippy/website verification deferred to hosted CI.

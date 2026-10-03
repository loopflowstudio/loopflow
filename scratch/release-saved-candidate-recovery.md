# Scheduled saved-candidate recovery — 2026-10-02

The bounded local integration is implemented. Scheduled retry inspects the saved
exact source under the existing target lock. An invalid candidate can advance
only after affirmative unpublished evidence from the configured publisher.
The attempt retains its rejected selection and inspection before selecting one
successor; original dues, frozen coverage, prior attempts and failures survive.
A valid candidate resumes unchanged. Unknown or partial publication fails without
replacement. A moved original tag also fails before authorizing replacement.

The port reuses `ReleaseSourceInspection`, `inspect_release_source`, and Python
`inspect_source` decisions from run-records at `6acd2389e878bbc47c1b80bdab8591ebf06a46fe`.
It retains this branch's caller-preserving fetched source and inherited locks,
its one artifact receipt type, and its UI-host/public proof requirements. Cached
packages now require successful preflight in a fresh `LF_HOME`. Main's combined
queue observation and queue-aware integration decision replace release's old
`GhPrView`; repair errors reobserve merged state. No controller or source checkout
was wholesale replaced, and run-records was inspected through Git objects only.

An interrupted authorization leaves the rejected selection pinned. The next
wake must inspect and authorize again; late writers cannot select or authorize,
and successor selection does not inherit the rejected candidate's workflow id.
Another invalid integrated successor fails with its exact selection retained for
the next wake. It cannot tag or settle merely because a predecessor was rejected.

## Focused proof

The Docker CLI test covers four scenarios: one verified successor, unknown
publication, partial publication, and unchanged valid-candidate resume. Historical
dues and the starting packaged-preflight failure are explicitly seeded. The real
cron executor, authored Flow, CLI, accounting, Git operations and file locks run
against disposable repositories/Homes; telemetry, hosted PR/build responses and
publisher proof are simulated. The test preserves the original failed attempt
byte-for-byte as JSON, original tag, covered dues and caller HEAD/index/staged,
unstaged, untracked and local-commit bytes, with at most one product settlement.

A separate process test kills its fixture controller while the inspector child
survives, proves target exclusion and caller preservation, then lets that child
exit naturally. This is local process evidence, not a production interruption.
The accounting test covers interruption before successor selection, retained
owner across another due, stale writers, and one replacement per attempt. Queue
proof covers queued/awaiting-queue distinctions and actual release rearming and
reintegration against simulated GitHub.

Validation: Docker `cargo test -p loopflow --test scheduled_release_tests saved_candidate_recovers_only_with_affirmative_unpublished_source_evidence` passed (four scenarios, 32.06s); `release_lock_tests surviving_source_inspector_excludes_replacement_after_controller_death`, the two queue integration cases, explicit-tag resume, incomplete-tag refusal and failed-tag preservation passed; focused accounting/queue/DTO lib tests passed; `uv run pytest python/tests/test_release_publisher.py -q` passed 30 tests; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, Python Ruff checks and `git diff --check` passed. The affected-suite gate remains with gate.

Native joined CLI attempts stopped before release dispatch because this branch's
legacy startup gate reads the OS account's installation receipt and rejects an
artifact set missing the Daemon role. Clearing inherited Home/binary variables
and building the daemon did not resolve it. The isolated Docker account supplied
the joined proof without editing installation authority or runtime stores.
A child-test completion marker initially followed a write to its dead parent's
pipe; the fixture now observes child completion without that unrelated SIGPIPE.
Older fake publishers now emit the new inspection response. No retries or
production test bypasses were added.

## Review and remaining work

Review found and fixed two preservation details: replacement must retain the
existing moved-tag refusal, and a new candidate must not inherit the old workflow
run id. Selection is now restricted to the exact Running attempt. Source
inspection stays with the publisher; accounting stores its authorization evidence
and the release controller remains the only successor/settlement owner.

This does not establish configured acceptance or completion of LOO-285. Full
main Session/Exec/Home/Flow integration, closed same-Home continuation, remaining
interruption cases and affected-suite verification remain. Preserve all 36
recorded telemetry failures, including the original 35. Actual telemetry,
UI-host/public-artifact proof and two adjacent configured dues settled by two
distinct automatic executions, including one publication without manual repair,
remain required. No new handoff or independent Task was created.

The initial file snapshot was retained outside the repository. Only the eleven
implementation/documentation/fixture paths for this contribution differ from
that snapshot; all other pre-existing files remain byte-identical. HEAD remains
`7458ab8d6cd920b1de1c391757a151d72ea638a9`, with nothing staged. No checkpoint mixed
this contribution with earlier dirty work. No Task sync, publication, landing,
PM change, installation, schedule change, production release, worker launch,
review completion or runtime-store mutation occurred.

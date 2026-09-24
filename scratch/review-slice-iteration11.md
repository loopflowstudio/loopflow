# Iteration 11 review — 2026-09-23

The slice advances the accepted design. Active now includes existing local
Task checkouts, as explicitly requested during the human demo, independently
of Session/provider gaps. The configured app exposes the exact current LOO-291
conversation after the concurrent shared discovery correction. This is fresh
read/display evidence; configured Session-row interaction and nested workspace
continuity remain open. Publication is not approved by this review.

## Evidence

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Stable Active membership | Existing local checkout, open Session or owned provider in checkout; completed planning work stays labeled | Shared optional local_exists, typed Session joins and owned Activity evidence; no sticky row cache | Fresh five-test Swift command; configured Task identifiers | pass, model and displayed membership |
| Unavailable evidence | Out-of-plan Work stays reachable; unknown is not healthy emptiness | Shared completeness check retains Wave heading; unknown worktree warning suppresses empty state | Active-empty parameterized regression; source | pass for out-of-plan regression; unknown-filesystem warning inspected only |
| Exact current conversation | Discover an interactive continuation even if originally headless; bind declared Task selector | Shared retained client history plus existing Work-binding resolver | Fresh aligned CLI reads and exact AX Session-row identifier | pass, discovery/display; opening not exercised |
| One Home | Planning, Sessions, Activity and actions address the same selected Home | Initial demo mixed development storage and production observation; concurrent relaunch aligns them | Selected path environment, CLI receipts, store-resolution source | initial gap corrected in later demo |
| Native row and nested navigation | Focus existing shell and retain both split levels, exact draft and companions | Existing ownership unchanged | Prior four-PTY fixture; prior configured launch stopped before row selection | gap for configured interaction |
| Performance/full Task | Scoped paint/readiness budgets, authorized directive edit and external trials; LOO-284 contract | Earlier count timings remain narrower; no new performance or external trial | Existing Task ledger and current Session DTO | gap |

Fresh command, exit 0: `swift test --package-path swift -Xswiftc -gnone --jobs 4
--filter 'WorkspaceNavigationTests/(worktreeRetainsActiveTask|activeEmptyRequiresCompletePlanning|workDoesNotRequireSessions)|DTOFixtureTests/(roadmapFixture|activityFixtureRoundTrips)'`.
Five tests pass, including both out-of-plan cases. [Receipt](configured-ui-evidence/iteration11-review/swift.log).
This covers the current Swift model and shared fixtures, including the local
worktree addition. No Swift source edits followed this review's test.

The existing six-test Rust activity receipt covers live receipt ownership,
versioned/app paths, checkout propagation, stale/dead receipt exclusion and
pruning. The concurrent discovery writer's one-test receipt covers ordinary
headless exclusion, interactive publication, client exit and explicit resolution.
Both receipts were inspected and copied into the evidence directory; neither is
a new Rust run by this review. No broader gate ran.

## Configured observations and corrected interpretation

At 04:49 UTC, passive AX inspection of demo PID 44263 exposed eight Task rows
and three Sessions. The eight rows matched CLI projection when the probe used
the app's actual environment. Seven had local checkouts; LOO-226 remained via
owned provider activity. The initial CLI probe omitted inherited LF_DB_PATH:
Session list rejected the production database and roadmap returned an empty
snapshot. That was a different environment, not proof that the app's Session
read failed. It is retained as an unsuccessful probe, not a passing comparison.

The app's selected path environment established the actual mismatch: LF_DB_PATH
selected the installed development database, while LF_HOME/LF_CONTROL_HOME and
LF_CONTROL_DB_PATH selected production. Registry planning and Sessions use
storage_config_from_env; Activity uses observability_database_path. Thus the
successful eight-row observation combined separate Homes. No authority guard
was bypassed and no installed/global configuration was changed here.

During review, the human-demo writer corrected discovery and replaced its own
demo with PID 41489. This review independently read the new process's selected
Home path variables and ran its three shared queries successfully. All selected
database/Home paths now name the installed development Home. The current Run
`run_643d86d70d5b4109a460e80a87a97533` is active and carries Task Work
`task_2aa71a7e36fe416d8a721e2b2f7c54e7`, exactly matching LOO-291's roadmap runtime.
At 04:53:27 UTC, AX exposed that exact Session-row identifier, seven Task rows,
and six Sessions in PID 41489. This confirms configured discovery and rendering;
it does not establish where that client's terminal is attached or prove a click.
[Corrected AX receipt](configured-ui-evidence/iteration11-review/observe-corrected.log),
[aligned paths](configured-ui-evidence/iteration11-review/corrected-app-home.json),
[shared Sessions](configured-ui-evidence/iteration11-review/corrected-sessions.json).

This review sent no input, activated no app, launched no provider, and opened,
moved or resolved no Session. The other demo's AX traversal hit its explicit
4,000-node bound on the last observation; it supplies no complete inventory.
Human confirmation remains pending. The earlier mixed-Home result is superseded
for the corrected demo, not erased. Binary/source hashes accompany both phases.

## Source and architecture

Recovered the complete tracked Task diff under `/tmp/loo291-review11-tracked.patch`,
compared every section with the preceding complete review, and inspected changed
source/contracts plus current untracked direction/evidence. The final comparison
is recorded in [diff.json](configured-ui-evidence/iteration11-review/diff.json).
Concurrent discovery and direction changes are included; this review does not
claim their authorship. No executable correction was made by this review.

Negative searches retain one Podium owner each for roadmap, Sessions and Activity
and one root window registry. SessionScope, SessionContext, SessionGroup,
SessionRowItem, PodiumConsole, PodiumSurface, _loadHierarchy and providerLaunch
are absent from reachable Swift source/tests. The Rust and Swift activity enums
now agree. Local checkout existence is an explicit optional shared field;
filesystem failures remain unknown and Swift adds no filesystem reader.

Session history and liveness retain different meanings. Both list and open now
use the retained interactive-history predicate; original launch provenance stays
unchanged. Declared issue/slug subjects use the existing binding resolver, with
unresolved subjects remaining unmatched. No cwd-based Session join, second
inventory, provider launch, or additional Session action matrix was introduced.
The existing LOO-284 shared legal-action/display fields remain absent.

Next configured proof should start from the aligned Home and a fresh disposable
conversation. Exercise its exact row, retained shell and nested worktree slots;
preserve the current human conversation. Then finish the remaining scoped
performance evidence. Directive editing, LOO-284 integration and human-selected
external trials remain full Task obligations. Review-slice permits publication
only “When all applicable `Done when` claims hold”; these gaps remain. No PR was
published, landed or completed, and the Task was not completed.

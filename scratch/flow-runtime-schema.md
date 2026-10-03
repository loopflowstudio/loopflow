# Flow runtime schema blocker

2026-10-02. Jack requested retry and unbreak of the existing pursue invocation
`f443e87f-be1c-4b85-bf9b-8d9593eb840d` through publication and demo.

## Observation

Retry with installed lf 0.12.29 fails at loop-decide before agent execution:
Codex rejects `maxProperties` in `codex_output_schema` (HTTP 400,
`invalid_json_schema`). The earlier Claude retry also rejected top-level
`anyOf` in the generated tool schema. Flow position, implementation and local
scratch remain intact. No decision or review completion was fabricated.

## Cause and repair evidence

Fix fac48dd22 (#1401) is already merged into this checkout. It uses required
nullable summary/reason fields, removes root unions and maxProperties, and
preserves evidence validation when decoding decisions. The installed release
predates this fix.

The initial latest GitHub Release was v0.12.29. Release candidate v0.12.30 at
8cbd0c5b1c99a12151908c59f923b0128706a34f built successfully in workflow
36994680041 but still contains the broken schema. Installing it would not repair
this failure. No later release-build artifact was found.

The installer refuses validation-only source promotion. Repair through the
production path therefore requires a subsequent patch release containing #1401,
installation, and retry of the same Flow. Jack subsequently authorized releasing
and installing the patch and completing recovery through demo readiness.

## Runtime recovery

The owning release operation published v0.12.30, then opened release PR #1407
for v0.12.31 with the schema fix. The operation runs the built CLI with an isolated
LF_HOME; it does not promote source builds into the production installation.
v0.12.31 is now installed; configured-provider replay succeeded.

The feature checkout is already marked resident in Git configuration. v0.12.31
repairs Flow execution but predates resident scratch handling. Feature publication
therefore uses the feature binary with an isolated Home (`LF_HOME` and `LF_BIN`
explicitly set), preserving the production installation and local scratch. The
source binary's `pr publish` supports that taskless publication path; the older
installed publisher would stage local scratch. This remains a separate concern
from installing the published runtime to resume the saved Flow.

Merge-group run 37072469684 failed `telemetry_flow_op_runs_internal_scorecard`:
the captured input had usage but no completion outcome after artifact deletion.
`SessionRecorder::drain_after_settlement` waits at most 250 ms and ignores a
timeout; the test deletes artifacts and reads SQLite immediately afterward.
This is a timing-sensitive assertion against asynchronous retention, not evidence
that the Flow schema fix failed. The release driver's automatic queue re-entry
passed as 37073021018. No manual check override or fabricated decision was used.

PR #1407 merged at 05d4dd1a0 and candidate build 37073560571 passed all platforms.
Artifact download then timed out against GitHub storage. Re-entering the owning
release operation selected newer main because preparation was incomplete; it opened
retry PR #1411. That candidate includes 36446011a (#1409), which repairs the observed
telemetry race by retaining terminal completion synchronously. PR checks passed;
merge-group run 37077184840 passed. The operation's tendency to select newer
main after a download interruption is an observed recovery limitation, not a
reason to fabricate candidate preparation evidence.

PR #1411 merged at a278d6bc1; candidate 37077794913 passed. Artifact download
completed. A concurrently active release driver held the exact preparation lease,
so this driver exited without modifying the owned state. The owning driver
completed publication at 2026-10-02T23:49:48Z. `lf install` then verified published
0.12.31 against the production store: exact frontier 0.12.31.001_release, all 33
executable references resolved, promotion accepted. `lf --version` reports 0.12.31.

Retrying the original Flow succeeded through the Codex decision boundary. The
provider returned Advance with implementation evidence, and the same Flow moved
from position 4 to 5 (`pr-publish`) with no failure. Publication uses the resident-aware
feature CLI; the following authored demo boundary remains open for Jack.

Check: `cargo test -p loopflow --lib engine::flow_output::tests` — 4 passed;
configured production replay remains blocked by the old runtime.

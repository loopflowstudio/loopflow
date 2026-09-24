# Joined release slice review

## Verdict

**Iterate. Keep the serial PR unpublished and the Task incomplete.** The current
slice advances the complete design: cron reaches the authored mechanical flow,
publication recovery checks and repairs the configured public stages, and the
joined fixture preserves caller work. Two history counterexamples found here
are repaired. Remaining execution and evidence gaps still prevent approval.

Starting head: `4a8a9f312ae57f3059482844e34aa9ea44a7846a`, with the preceding
compression report uncommitted. Obtained the complete 462,597-character patch
with `lf task diff LOO-285 --json` (`truncated: false`). Reviewed the directive,
full design/Done when/forbidden outcomes, changed production owners, their CLI
and JSON consumers, publisher and integration fixtures, and retained evidence.
The earlier review remains historical; this report updates its conclusions for
the joined execution and publication-recovery cut.

## Demonstration

`cargo test -p loopflow --test scheduled_release_tests -- --nocapture` passed
the real cron executor → built CLI → authored Flow → release operation → atomic
settlement → history path in a disposable Home/repository. Its five cases cover
publication, no-change, rejected telemetry, post-publication smoke failure, and
missing public-stage proof. Each retains caller HEAD, branch, byte-identical
index, staged/unstaged content, untracked files, and an unpublished local commit.
Execution took 39.68 seconds after compilation.

This is production-like local execution. Telemetry, GitHub, and the publisher
proof in that Rust fixture are simulated; dates are synthetic. Python recovery
counterexamples separately execute temporary installers and binary smoke while
mocking public services. None is an installed automatic settlement, real UI-host
gate, signing run, or public artifact download. No production lifecycle or
publication action was taken to manufacture evidence.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| Configured mechanical target | Authored release flow wins; missing flow cannot fall back to skill | Shared discovery gives repository flows precedence; declared cron requires Flow | Joined CLI execution; catalog source and existing regression | pass locally |
| Stable due identities/repeated firing | Original dues retained; one settlement per owner | Deterministic obligation/due keys and idempotent settlement | Accounting source and retained focused tests | pass locally |
| Delayed wake/frozen coverage | Misses share one execution; newly due work waits | Atomic obligation document contains owner links and saved covered keys | Joined fixture has at least three dues and one attempt; existing delayed-wake case | pass locally |
| Historical intervention and timing | Collapse cannot erase manual provenance or first attempt | History now considers collapsed provenance and earliest covering attempt across retained attempts | Two new regressions failed before repair and passed afterward | pass locally |
| Same candidate through preflight/retry | Recovery retains owner/tag/commit and failures | Prior selection copied through preflight; exact candidate checked before completion | Earlier regression/source; joined post-publication failure retains selection | pass locally |
| Late process/settlement result | No terminal regression; rejected evidence retained | Exact attempt fencing and atomic outcome/proof; wrapper cannot overwrite success | Settlement source and prior focused preservation test | pass locally |
| Process zero exit | Cannot establish publication alone | Missing product settlement remains Unverified | Cron writer/source and prior shell fixture | pass locally |
| Failed telemetry | Scheduling success does not clear failures or permit publication | Current telemetry gates selection; failures and disposition ages remain visible | Joined failed-telemetry case; retained 36-failure Home observation | pass for gating; repair ownership gap |
| Telemetry for each original covered due | Preserve historical prerequisites and current recovery linkage | Only current due and a two-day receipt search; no bounded prerequisite retry | `verify_scheduled_telemetry`; pending follow-up | gap |
| No-change/exact-candidate checks | Empty captured range, verified baseline/current checks | Exact range rechecked; baseline public proof and current checks required | Joined no-change; earlier wrong-source regression; source | pass locally |
| Complete publication/reconstruction | Exact assets, all configured stages, required UI and smoke | Reads immutable and latest endpoints; repairs demonstrably missing/stale stages; refuses conflicting bytes, unavailable authority, newer tag | Eleven focused Python public-proof/recovery cases | pass locally; live proof gap |
| Post-publication failure | Preserve external effects without verified success | Repair stages retained before later work; smoke failure cannot settle success | Joined smoke failure and Python real failing installer | pass locally |
| Overlap/parent death | One mutation owner through surviving children; exact continuation | Target/job locks exist; Git/PR subprocesses still omit descriptor inheritance; some continuations remain prose | `ReleaseLock::inherit`, tag/PR call paths and shared command helpers | gap |
| Changed/removed obligation | Retain denominator and actionable unfinished work without transferring Home authority | Closed segments retained; successor link survives interrupted rewrite; unfinished old attempts can remain Running | `observe`/`close`/`receipt_context`, prior replacement test | gap in continuation |
| Corrupt persistence/DST | Corruption is failure; no invented local due time | Strict parsing, atomic replacement, shared first-ambiguous/skip-nonexistent calendar | Source and retained corruption/DST cases | pass locally |
| Caller/interruption/isolation matrix | Caller bytes survive every exit; independent targets/repos do not collide | Five joined outcomes preserve bytes; canonical repo/target lock; full kill and cross-repo matrix absent | Joined fixture; lock/source identity review | partial; gap |
| Two adjacent automatic settlements | Two distinct automatic executions, at least one publication, no repair | Summary rejects known ineligible evidence; no configured pair demonstrated | New provenance regression and retained Home history | gap |

## Reproduced and repaired

1. A triggered attempt failed before selection, then a later scheduled wake
   collapsed its opportunity into a new owner. Qualification examined only that
   owner's attempts. Two later adjacent successful owners therefore produced a
   false unattended pair, even with the triggered failure retained outside the
   report window. Qualification now checks the owner and all opportunities
   coalesced into it for operator provenance. The regression covers Triggered
   and Manual sources, and still accepts equivalent fully Scheduled catch-up.
   Product counts remain one publication and one no-change in all three cases;
   only eligibility changes. No provenance copying or new state was introduced.
2. Timing looked only at attempts on the current owner. An on-time failure
   followed by collapse was relabeled caught-up. Timing now selects the earliest
   retained attempt whose frozen coverage contains the original due key. The
   regression retains two on-time entries and zero caught-up entries after
   collapse. Original failures and settlement cardinality remain unchanged.

Both defects were visible in the read projection; persisted records already
contained the necessary evidence. No DTO, schema, migration, or historical
receipt rewrite is needed. User-facing cron history documentation now states
that collapse preserves both timing and intervention provenance.

## Negative architecture and remaining direction

Searches still find no release-selection `sync_main`, duplicate successful
evidence wrappers, `record_verification`, or separate Python candidate/publish
receipt classes in the inspected production paths. `settle` remains the sole
typed product writer, and the repository flow contains one mechanical op. The
new history fixes consume existing evidence and cannot start or settle work.
No new daemon, scheduler, SQLite table, recovery platform, or compatibility
adapter was introduced.

Do not treat local success as full-design acceptance. Continue in this Task:
retain each original telemetry prerequisite and resolve bounded recovery;
carry exclusion through every mutation child and prove actual parent death;
give closed unfinished obligations an explicit disposition/continuation;
finish interruption/isolation proof before supported installed acceptance.
The observed scorecard query against missing `agent_turns` remains a verification
blocker with an unaccepted Intelligence handoff. Required UI-host proof and two
adjacent automatic settlements remain mandatory. No independent new live
publication failure was demonstrated and no sibling Task was opened.

## Validation

- Joined real-CLI fixture: passed, five simulated external-outcome scenarios.
- `uv run pytest python/tests/test_release_publisher.py -q -k 'public_proof or reconcile or smoke_failure'`: 11 passed, 9 deselected.
- `cargo test -p loopflow --lib ops::cron::history::tests`: initially 2 passed/2 failed; after repair all 4 passed, including unchanged DTO round-trip and prior rejection cases.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`: passed on the final repair, including the regression fixture's retained observation interval.

The joined demonstration preceded the history-reader repairs. Their final proof
is the focused history test run; the unchanged execution/publisher paths were
not rerun for ceremony. No affected-suite gate, full CI, installed cutover, or
live acceptance is claimed.

# LOO-371 — October 4, 2026

Direct Task routing is changed locally; end-to-end acceptance is **not established**.
The first implementation removed the observed Task-link sheet, but its latency
run overlapped an interrupted benchmark and then encountered severe host load.
No speedup or latency-budget pass can be inferred from this comparison.

Jack Heart requested autonomous reproduction of LOO-368 against a representative
isolated Home. The SQLite online backup preserves 321 Tasks, 175 conversations,
123,217 Session events and 116,688 Execs. LOO-368 had nine associated conversations:
eight headless and one interactive, all unfinished. No history was deleted.
The frozen snapshot hash and complete table counts are in `snapshot.json`.
The private database remains at `/tmp/loo371-frozen`; it contains credentials and
must not be committed or published. Each invocation used a separate copy.

## Recorded observations

The machine was an arm64 MacBook Pro, macOS 26.0.1, with 16 logical CPUs.
Swift was built with SwiftPM debug `-gnone`. The baseline used the installed
release `lf`; the first implementation used a local debug `lf`. Binary hashes,
source hashes, host identity, reads, transitions and failed attempts are retained
in each directory. This build-mode difference is another comparison limitation.

| Scenario | Baseline first / subsequent median ms | First implementation first / subsequent median ms | Successes / attempts, before → after |
|---|---:|---:|---:|
| Cold workspace construction | 13,968 / 20,082 | 17,318 / unavailable | 3/3 → 1/3 |
| Warm Task open | 4,815 / 5,920 | 6,760 / unavailable | 3/3 → 1/3 |
| Reopen Task | timeout / 7,625 | 725 / unavailable | 1/3 → 1/3 |

The repeated baseline has seven successful endpoints and two 45-second timeouts.
The first implementation has three successes and six timeouts, including records
written after launcher interruption. **p95 is unavailable**: no comparable group
has 20 successful samples. Subsequent groups contain two attempts each; first
interactions are kept separate. Durations include capture/OCR observer cost.

All nine repeated baseline attempts observed the Task-link sheet and up to two
visible windows. The first implementation sampled one visible window and no
Task-link sheet in all nine attempts. Key-window observations were always `-1`;
focus and usable input are not proven. Transition polling is not frame capture
and cannot exclude every intermediate screen. In particular, the cold path still
constructs a workspace before Task resolution; absence of an unrelated initial
landing screen has not been established.

`baseline-interrupted/` preserves the earlier 21-sample invocation, including its
late writes: 63 attempts, 37 successes and 26 timeouts. Its launcher returned -15,
but the SwiftPM test child continued writing until 00:28:49 Pacific. It overlapped
later runs. The repeated baseline and after run are retained as observations,
not an uncontaminated baseline. Reports were regenerated from the final journals;
the original launcher metadata remains unchanged.

The coordinator interrupted the work after host load reached 292. At continuation,
free disk was 28 GiB, below the recorded 32 GiB reserve; load was still elevated.
No further build or timing run was started. Those are concrete capacity blockers,
not proof that the app meets its targets.

At the October 4 11:00 continuation, free disk was 12 GiB and one-minute load
25.44. The 32 GiB reserve still prevents fresh native builds and measurements.
No comparable rerun was attempted, and no live Home or provider was changed.

On October 5, free disk was 135 GiB and one-minute load 120 on 16 CPUs. Disk no
longer blocks; load still contaminates timing, so no rerun was attempted.

## Targets and changes

Before production UI changes, the exploratory baseline (cold 12.1–15.2 seconds,
warm 4.5–4.9 seconds and a reopening timeout) informed provisional targets:
**cold workspace ≤5,000 ms; warm/reopen ≤250 ms; no extra successful-path window
or Task-link sheet**. These targets remain unmet/unproven. No Session-readiness
budget was scored without a credible baseline.

The branch removes successful-path loading sheets, reuses already observed scoped
Task/Session evidence, preserves a Task's focused Session, and prefers a window
already selecting the destination. Unknown Session links still enumerate every
page: a partial inventory must not remove retained panes on later pages.
Missing/ambiguous links retain their error/choice sheet and generation fencing.

Profiling a Session read found 333 Git checkout-root probes, 305 for missing paths.
Canonicalizing before launching Git reduced the observed root probes to 29
(336 total Git invocations to 31), without removing historical records. The
wrapper adds observer cost, so these counts establish avoided work, not native
latency. LOO-304 retains polling/projection work; LOO-372 retains ordinary
interactive, unfinished Session filtering.

Compression removed the unsafe partial-inventory shortcut and covered planning
versus runtime Task identity when reopening. The measurement runner now builds
first and launches SwiftPM's native test helper directly in its owned process
group; a no-test probe confirmed that ownership. Per-sample reads drain after
window teardown, and read termination is requested after 30 seconds. These
runner changes require a new baseline: neither archived run used them.

Continuation review found SIGTERM could still orphan the owned helper. The runner
now handles SIGTERM and timeout for both build and measurement, terminating the
group even if its leader exits first. Two real subprocess tests prove termination
and timeout outcomes with no surviving test child; all 13 Python tests and Ruff
pass. Native execution of this revision remains unverified.

## Remaining acceptance

Once a capable host has capacity, compile the latest changes, run the focused
destination tests and Rust Clippy, then repeat baseline and optimized runs with
the same harness, snapshot and CLI build mode. The latest Swift retention and
runner revisions have syntax checking only; earlier destination code passed 16
tests and the Rust resolver passed two focused tests. Python's 13 focused tests,
Ruff and Rust formatting pass. Gate owns the broader affected suites.

The current snapshot runner hosts production Podium, routing and local CLI reads,
but it does not exercise OS app launch/URL delivery or usable provider input.
Copied provider actions are refused. Add isolated Session deep-link and retained
input proof, actual cold application launch, and reliable native focus observation.
Cold direct-open behavior still needs that proof and integration with LOO-376,
which now owns cold bootstrap, persistent caching and unified refresh orchestration.
LOO-291/LOO-353 acceptance and sustained-use KRs are unchanged.

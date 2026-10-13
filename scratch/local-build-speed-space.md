# Local test speed and build-output space

Task lf-8f870b1 · Reconciled October 13, 2026: practical cleanup policy accepted.
Jack Heart accepted the outcome and delegated technical choices and autonomous
delivery. These mechanisms are implementation decisions, not measured gains.
Slice 1 implements exact-CLI preparation independently. The native Cargo counterexample
limits the supported concurrency contract; it does not authorize deleting busy work. The accepted outcome still requires
both measured speed improvement and bounded, safe build-output lifetime.

## Problem and demo

Developers pay repeated Cargo startup and CLI resolution costs inside local test
processes, while completed checkouts retain gigabytes of regenerable output.
Preserve useful tests, warm builds, debugging information and retained source.

The Rust phase of `scripts/test.py` prepares its exact materialized CLI
once for CLI-heavy fixtures. Cleanup remains proposed:

```sh
lf wt prune --builds --dry-run  # proposed: eligible output and retained reasons
lf wt prune --builds            # proposed: reclaim output, retain checkout
```

Old terminal output becomes eligible even above 64 GiB free disk and below 24 GiB
per checkout. Source, dirty/untracked edits and registered checkouts remain.
Existing resource recovery calls the same deletion owner and reports its receipt.

## Evidence and scope

[Research](local-build-research.md) owns the baseline, owner map and native-lock
probes. Baseline source: `f2855f7bd46aa93d57b3a2e56fc8ec917f266ea3`.
The pre-compression design is preserved at
`44ac1ec24242e22ecf5f77ef959d29f9609716df:scratch/local-build-speed-space.md`.
The research's ordered fixture pair was only a hypothesis. Slice 1's final matched
evidence now lives in the [storage review](../docs/reviews/storage-footprint.md#local-verification-cli--2026-10-13).

Infrastructure's local-development friction and build_disk_bytes apply; no numeric
Task KR is supplied. Authored objective and memory now live in
`.lf/wave/infrastructure/`; the supplied `wave/infrastructure` pointer predates
the relocation in #1536. Its immediate child Release's goal and complete memory
were read; no other immediate child memory exists in this checkout. Release's
operation-entry lesson requires proof through prune/recovery, not only selectors.
LOO-390 owns history/install retention, lf-6d3ff35 owns test
deletion and lf-12be237 owns the ten-real-run CI proof. Exact sibling Task input
confirmed lf-6d3ff35's separate scope; broad plan inspection failed, so other
overlap remains unknown.

## Writer exclusion: accepted practical policy

Jack Heart accepted the smaller practical policy on October 13: do not build
extra machinery for arbitrary direct Cargo starting cleanup/build in a completed
worktree during reclamation. Jack Heart emphasized that a Cargo process in a
worktree selected for deletion would raise a candidate-selection concern.
Eligibility and preservation of detected activity are therefore the priority.

Require positive completed-Task ownership, no unfinished association, old
regenerable output, fresh execution/process evidence and the existing worktree
lease for cooperating operations. Missing or failed ownership/activity inspection
retains output. Keep primary/current/persistent checkouts and active or uncertain
work. Detect cwd, executable mappings and open files; a binary running with cwd
elsewhere must retain its output. Preview explains candidates and retained reasons.
Re-read eligibility under the lease immediately before mutation. Verify the actual
prune/recovery path with disposable state and real process/lease counterexamples.

Cargo 1.98.1 clean unlinks a held profile lock, allowing a replacement build. This
remains contrary evidence against universal native exclusion, not an outstanding
approval blocker. No snapshots, rename or native-profile-lock scheme may claim to
solve it. Concurrent manual build/clean/package-manager operations ignoring the
cleanup lease in a completed candidate are unsupported and must be documented.
Existing detected processes and uncertain facts still preserve output; no universal
race-free guarantee is claimed. Native lock probes may inform implementation but
perfect exclusion of arbitrary direct tools is no longer a prerequisite.

Do not add mounted filesystems, a cache daemon, an upstream Cargo lock protocol or
a generic retention framework for this rare race. Preserve the research receipt.
The existing Python cwd/age-only cleaner still lacks positive completed ownership;
replace its build deletion through the supported owner in the same cut.

## Implementation and remaining design

### One prepared verification CLI

Implemented `--prepare-cli`, selected by test.py's Cargo/nextest Rust phase.
Exact-tree copying and migration materialization precede one
`cargo build --offline -p loopflow --bin lf --tests --message-format=json`.
Build selection (toolchain, configuration, target, profile, features, jobs) follows
the verification command. The executable comes from the one non-harness `lf`
Cargo artifact. Missing, ambiguous, unavailable or failed preparation stops tests;
LOOPFLOW_TEST_LF replaces inherited candidates with no installed/source fallback.
Generic commands remain unprepared unless requested; direct-test support remains.

October 13 implementation decisions under Jack Heart's technical delegation:
`--locked` failed because materialization advances workspace versions without
updating Cargo.lock. Offline preparation permits Cargo to update only that
disposable lockfile, matching the existing fixture path. Ordinary CLI preparation
then repeated the root build across different dependency graphs: exploratory
warm total 103.951s versus a fallback run's 104.360s. Preparing test targets instead
keeps the verification graph. Three final alternating pairs improved total median
113.050s → 79.716s (29.5%); all six passed both fixtures. Preparation is included.
The verification root retained 4.492 GiB, entire Rust target 10.101 GiB and
Rust + Swift 10.890 GiB. Isolation adds output rather than reclaiming it.
The storage review owns full ranges, allocation counts and raw command references.

Default output is target/materialized-tests. Explicit CARGO_TARGET_DIR or
--target-dir names its parent, resolved against the originating checkout before
changing cwd. The nested verification root replaces source-target sharing.
Materialized invocations remain serial in one checkout. Preparation/execution
clear inherited LF_* authority and close stdin. Prepared mode explicitly sets
development/validation_only and removes release tags; fixtures retain independent
Homes and the network runner. Four jobs remain the default.

Nextest still performs its cheap warm build check; no unsupported --no-build or
archive path is added. Isolation adds retained output: source, materialized and
app-control allocations are counted together. This slice claims no disk reduction.

### One build deletion owner

Extend lf wt prune with --builds, --json and bounded policy arguments for retention
hours, maximum roots and bytes. Defaults: 24h, eight roots, 64 GiB. Resource recovery
passes its existing policy and remaining budget; cache and build actions retain
current per-operation caps. No new retention table, journal or background service.

Build mode bypasses main refresh, branch deletion and ordinary checkout pruning.
Extend the existing read-only SQLite ownership reader in place to return positive
completion evidence as well as unfinished ownership. Preserve its released-schema
handling; do not migrate the installed store. A root qualifies only with a recorded
Done Task, no nonterminal Task association, and idle associated execution. Missing
Home/Task, abandonment alone, incompatible schema, unresolved execution or failed
ownership reads retain output. PR-less completed Tasks can qualify. Never infer
completion from branches or missing PRs. Primary/current/persistent checkouts stay
protected. Selected and production registries must both preserve unfinished work.

Known generated roots: target, .build, swift/.build, website/node_modules. Require
ignored generated ownership; tracked content inside a root, symlink escape or
uncertain contents retain it. Ordinary dirty/untracked source outside output
survives. All output modifications must be older than 24h, independent of size or
free-disk thresholds. Lifetime is operation-bound: eligible on the first supported
prune/recovery after grace, not wall-clock expiry without invocation. Capped calls
may require repeats to drain a backlog; recent/active output keeps its warm state.

Acquire the existing nonblocking worktree lease before mutation, then re-read Task
and execution facts and inspect cwd, executable mappings and open output files.
Failed/timed-out/incomplete inspection retains output and explains why. Normalize
/tmp versus /private/tmp. A running executable survives even with cwd elsewhere.

Return candidates, retained reasons, removed paths, allocated bytes and failures
in a required/optional JSON DTO. Python retains inventory and gate/uv-cache recovery,
and delegates all build deletion to this owner. Unavailable capable CLI retains
builds with actionable reporting; no Python deletion fallback. Candidate CLI reads
production ownership only; verification registries remain disposable. No Desktop
UI change; fixture any wire DTO in each actual consumer.

## Alternatives, exclusions and failure

- Cross-worktree mutable sharing is rejected by frontier and binary-race evidence.
  Prepared-CLI reuse is the selected hypothesis; no compiler-cache daemon or paid
  service without evidence that the simpler path is insufficient.
- Profile tuning/incremental is not selected: debug=1/incremental=false already
  bounds output. Losing dependency debugging or adding retained state needs its
  own matched win; warm preparation is already cheap.
- Source/app-control reuse is not selected: provenance/authority overrides and
  differing feature graphs make apparent duplication insufficient evidence.
- Pressure-only retention leaves small completed outputs indefinitely. Age-only
  cleanup without positive completion and writer exclusion is unsafe.
- Exclude history/store cleanup, real-Home migration, installation, broad test
  deletion, new lifecycle authority and a benchmark framework.

Success means independent fixtures without repeated preparation and inexpensive
retained finished source. Failure means fast tests using the wrong frontier or
cleanup deleting a running binary/new writer's output. Those block delivery even
when elapsed time improves.

## Delete — do not maintain

These cuts follow the accepted practical policy; preserve consumers until each cut
provides its replacement. Do not extend the predecessor to implement reclamation.

- In `scripts/resource_envelope.py`, replace the build branch of recover_resources
  with Rust receipts; delete _remove_build_artifacts, _build_cache_is_stale and
  its exclusive _raise_walk_error callback. Keep gate/uv cleanup, inventory,
  pressure reporting and overall caps. There is no Python build-deletion fallback.
- Remove cwd-only deletion authority from _running_cwds and
  `rust/loopflow/src/lf/commands/top.rs::running_workspace_paths`. Keep Python's
  inventory use; cleanup consumes explicit unknown/error facts and the richer
  process inspection. Cut over protected_worktree_paths, Rust's sole caller.
- In `python/tests/test_resource_envelope.py`, replace deletion fixtures based
  only on synthetic inactive ResourceSource values: test_recovery_removes_only_inactive_allowlisted_builds,
  test_recovery_root_limit_is_a_hard_bound and test_cleanup_reclaims_stale_builds_before_emergency_reserve.
  Carry source/history preservation, caps, age and pressure assertions into the
  Rust-backed path. Adapt mixed gate-capacity, busy-cleaner and busy-uv tests;
  keep their independent gate/cache guarantees and active-build preservation.

Slice 1 has replaced the source-target default and TESTING.md's shared-target
warning/recovery recipe. Exact-tree copying, failure cleanup, network isolation,
app-control output and support-crate direct-test preparation remain intact.

No new lifecycle store, clean-by-absence, incompatible mutable sharing or unlinking
held locks. Allocation counts are not exclusive APFS physical savings; selecting
zero tests is not proof.

## Current execution boundary

The prepared CLI Flow finished successfully as dfc7550eaf. Jack Heart subsequently
accepted the practical concurrency policy above, unblocking reclamation. No PR
lands or Task completes on CLI preparation alone. Remaining work is reclamation,
whole-outcome validation and delivery; research and kickoff need no replay.

## Internal slices and Done when

1. **Independent implementation and matched warm measurements complete.** The full
   Rust runner prepares the exact CLI with test targets in a separate verification
   root. Executable probes cover dirty/untracked/frontier fidelity, stale candidate
   replacement, authority overrides, target selection, independent Homes and
   preparation failure cleanup. A real-Cargo probe also checks copied-input
   fidelity and source-lockfile preservation with an explicit offline option.
   Three final pairs passed real local-planning and
   startup fixtures without leak warnings; an earlier exploratory trial's warning
   remains unresolved. The measured graph-sharing alternative meets the draft
   20% warm speed target. This slice alone does not meet Done.
2. Apply the accepted practical concurrency policy above. Preserve the native Cargo
   counterexample as a documented unsupported race; prove correct selection,
   observed-busy retention, inspection failure and participating-lease exclusion.
3. **Unify safe reclamation under the accepted policy.** Implement positive read-only ownership,
   native/process preservation and bounded build-mode pruning; replace Python
   deletion in the same cut. Focused disposable-registry tests retain a dirty Done
   checkout with generated output. Preview changes no content; apply reduces
   allocation while source/untracked bytes, history, stores and registration survive.
   Cover pending/unknown Task facts, incompatible registries, inspection failure,
   executable launched elsewhere, open files, held leases, cooperating writers starting after
   selection and detected direct-process retention. Use test-only race hooks plus real writer
   subprocesses; no production factory abstractions.
4. Remaining matched measurements and durable guidance. The warm CLI pairs and
   full retained-output counts are complete; TESTING.md and the storage review
   own operating guidance and results. Matched cold preparation, small edit/test,
   Swift/control cold/warm and reclamation remain. Hold toolchain/lockfile and four
   jobs fixed; include CLI preparation in measured totals.
   Report median/range, raw command references and compiler/setup/execution/cleanup
   separately. Draft technical targets, not Jack Heart's numeric acceptance:
   CLI-heavy warm median at least 20% faster; investigate >10% warm edit/app
   regression; actual disposable-output cleanup removes at least 90%
   of allocated bytes; native lock scaffolding may remain. Measure filesystem-free
   change separately and disclose APFS sharing/noise. If gain misses, revise the
   mechanism in place before delivery; Task outcome is not waived.

Gate commands, headless, once for the affected final content:

```sh
uv run pytest python/tests/test_materialize_rust_tests.py python/tests/test_resource_envelope.py
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
uv run python scripts/materialize_rust_tests.py --prepare-cli -- cargo nextest run -p loopflow -E 'test(build_cleanup) | test(local_planning::public_local_task_places_and_runs_without_a_planning_provider) | test(agent_startup_tests::)' --no-tests fail
swift build --package-path swift --jobs 4
uv run python scripts/loopflow-dev.py bundle-control-tools --output /tmp/loopflow-build-demo
```

Rust commands run with inherited LF_* unset and closed stdin. Apply the exact Cargo
runner configuration from scripts/test.py to the focused command for test_network.py
isolation. The proposed build_cleanup tests cross CLI → read-only store → cleanup
→ JSON → Python receipt, rather than only testing selectors. Include changed app
launcher lease tests in Python and existing worktree/prune preservation tests in
Rust. The resource recovery acceptance must prove eligible cleanup above pressure
thresholds and repeated invocation idempotence. Full hosted CI remains required.

Final measurements/usage belong in TESTING.md and docs/reviews/storage-footprint.md
before scratch cleanup. Done when normal local Rust verification demonstrates a
matched fixture speedup, old completed inactive output is reclaimable through
preview/apply and normal recovery with preservation proofs, affected/full hosted
checks pass, and the PR is verified merged and Task completion recorded.
Reclamation, final acceptance, full hosted CI and delivery remain unproved.

Compression/review removed the shared-target default, stale overlap/recovery docs
and hand-mirrored LF authority list. It corrected the prepared CLI's graph switch
and inherited release authority before measurement. Direct-test preparation and
all cleanup predecessors remain with their existing owners; no deletion mechanism
was extended or certified.

Compression keeps authority filtering in one environment construction and removes
test reporting that only rechecked TemporaryDirectory. Target rewriting now stops
at the harness separator, matching preparation's build-option parsing; harness
arguments remain verbatim. The real-Cargo probe validates the surviving artifact
path. Existing matched measurements still apply to the production runner, which
has no harness target argument; no CLI preparation or benchmark was repeated.

Implementation check: `uv run pytest python/tests/test_materialize_rust_tests.py python/tests/test_gate_bounded.py -q` — 58 passed; Ruff and diff checks pass; six final measured fixture runs pass; affected/full gate and cleanup proofs remain with later steps.

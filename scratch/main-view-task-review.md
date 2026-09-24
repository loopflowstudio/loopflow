# One outline and retained Task workspace — gate review

Gate disposition: **not ready to ship**. This pass repairs documentation and
prompt-snapshot drift and prepares review copy. The affected suites did not start
because the repository resource preflight failed. Core implementation and
acceptance gaps below remain explicit; no publication or Task completion follows.

## What was implemented

The branch replaces competing navigation with one repo → Wave → Task → Session
outline, presented in full, compact or flat Session form. Tasks restore retained
Monitor, Session and companion shell panes through the existing multiplexer.
Shared Rust reads provide exact active Run attribution, required Session → Run
links and human action descriptors; Swift retains presentation and native input.

The integrated chapter change replaces public Project navigation with direct
Wave Tasks and internal current-chapter ownership. It includes deterministic,
resumable chapter rotation, Task continuity, historical reads, CLI/DTO migrations
and updated agent instructions. The desktop runner records capture/text-verification
and PTY-input measurements for two fixed populations.

## Key choices and how it fits together

Rust owns planning, chapter membership, Session legality, Run identity and process
observation. Podium owns shared reads; repository/window navigation retains selected
Work and pane choices. Checkout multiplexers own pane layout, and the window owns
native surfaces. Project provenance remains readable without a Project navigator
or operator. Monitor selection is passive and shares one Home reading.

Native receipts remain discoverable without a new launcher marker. Capture bindings
cover generic Exec intervals, while native receipts cover interactive clients;
merging those lifetimes would lose existing clients. Failed observations retain
explicit gaps. Prepared Runs and matching checkouts are not liveness evidence.

## Gate findings repaired

- The architecture map dropped retained `project_events`, the Wave observation
  outbox and existing shell/tmux subprocess edges when removing the Project
  operator. Restored their actual owners without restoring that operator.
  Architecture coverage now passes all eight categories, including 31/31 tables
  and 27/27 subprocess edges.
- Regenerated `docs/architecture.html` from current documentation. Its consistency
  check now passes; generated website docs were synchronized.
- Corrected Swift README references to Project conversation scope, compact Project
  leaves, Project navigation and independent Project processes.
- Six prompt goldens still embedded the removed Project commands. Replaced only
  their operating-document fragment with the current builtin and corrected three
  grammatical remnants in that builtin. Exact fragment checks pass and all other
  golden content is unchanged. This is a bounded snapshot repair, **not** a full
  prompt-engine regeneration or test pass.

## Verification and evidence

Starting HEAD: `909adff3e0852c4512ff673a5ebefb75a8e18fc0`.
[Gate receipts](gate-evidence/receipt.json) and logs preserve current results.

| Check | Result |
| --- | --- |
| `scripts/test.py --reuse-passing` | Resource preflight failed; architecture, Python, Rust, website and Swift suites did not run or reuse a pass |
| `scripts/resource_envelope.py --recover` | Still blocked: main's active build is 19.1 GiB / 12 GiB; retained without deletion or bypass |
| `scripts/check_architecture.py` | Failed before map repair; passes afterward |
| `scripts/check_swift_multiplatform_boundaries.py` | Pass; static boundary check only |
| `scripts/check_migrations.py` | Pass: one chapter draft; 51 shipped migrations unchanged since v0.12.20 |
| `cargo fmt --all -- --check` | Pass |
| Generated architecture HTML check | Failed before regeneration; passes afterward |
| Six embedded golden fragments / untouched remainder | Pass by exact text comparison; engine test remains unrun |
| `git diff --check` | Pass |

Clippy, materialized Rust tests, full Python/Swift/website suites and Xcode fallback
compilation remain unverified for the final tree. Hosted UI behavior and fresh
native captures were not attempted: this invocation has no rendering environment.
E2E and hosted matrix execution remain CI/required-host work. The materialized
Rust runner also creates a raw temporary Git worktree; it was never reached.
Any subsequent run must reconcile that helper with the active `lf wt` requirement.

The [archived native baseline](../.lf/evidence/desktop-performance/20260924-capture-input/README.md)
retains five matching artifact hashes and 462 recorded passing observations.
It is prior evidence, not this gate's test pass. Representative warm p50/p95:

| Scenario | Small, ms | Large, ms |
| --- | --- | --- |
| Full hierarchy | 261.4 / 322.3 | 261.6 / 377.9 |
| Active Monitor | 269.8 / 868.2 | 363.4 / 859.8 |
| Retained Session return | 298.8 / 352.3 | 326.4 / 495.6 |

These include forced bitmap capture/OCR and input-observer cost. No optimized
before/after comparison, compositor latency, frame-hitch result or budget is
claimed. Concurrent edits strengthen report integrity in the Python runner/tests
and performance README; they remain the other contribution. Its now-complete
[review receipt](review-desktop-performance.md) records eight passing report tests,
22/22 short native capture/input observations and unchanged reconstruction of the
462-attempt baseline. All three source and seven artifact hashes match. This
supplementary receipt is not an exact-tree affected-suite pass; its complete tree
fingerprint predates the concurrent documentation edits.

Review scope reused the recorded chapter, Wave-integration, Session/Run and Monitor
reviews for unchanged implementation. Examined current active discovery, Podium
read ownership, Task pane restoration, Monitor presentation, chapter storage
references, measurement/reporting and shared build guidance. HEAD's non-evidence
delta since the prior integration checkpoint is direction/memory documentation.
This is not a claim of fresh line-by-line review of every historical source change.

## Risks and remaining Done When

1. Active native discovery still walks retained Run directories. Bounded discovery
   and shared automatic refresh remain core work; demand Refresh is the current UI.
2. Wave objective / Project metric-target ownership correction is not integrated.
   Current metric contracts still own targets; closed chapter target/evaluation
   continuity is not established by the earlier chapter proof.
3. Neither experience has a production rendered/usable latency or frame-hitch
   signal. Implement correlated input → visible usable outcome intervals for
   `hierarchy_interaction_ms` and `task_workspace_ready_ms`, including scrolling
   during refresh and configured registry cost. Publish budgets from baseline
   before scoring the required twenty long-lived-registry trials.
4. Cross-repository flat Session coverage, narrow combined-pane readability and
   fallback compilation retain recorded gaps. Five-column empty-state clipping
   has not been repaired or reclassified as a pass.
5. Human confirmation of the simplified canvas, configured positive Run changes,
   combined-pane provider input, the human-selected external workflow, authorized
   directive edit and ten external trials remain required. Self-hosting and
   fixture measurements do not establish Product's external usefulness objective.

## What's not included

No live chapter migration, installation, provider transfer, PM mutation, publication
or completion was performed. Run history/output/throughput remains later Monitor
work; LOO-293 is preserved. The two optimization Tasks stay deferred until the
core contracts and measurements settle. Live migration still needs its concrete
accepted preview; code integration does not authorize backlog retirement.

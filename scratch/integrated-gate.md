# LOO-298 integrated gate

2026-09-30. Jack Heart's supervising session authorized the integrated gate and
`lf pr land -c` after main integration at `19d521843`. Installation and migration
of the installed Home remain excluded.

## Review findings

- Session cursor pages selected by stable ID but sorted their projected rows by
  title. Renaming could change the returned cursor and repeat or skip Sessions.
  Preserve ID order through projection for cursor pages; ordinary inventories
  retain title order. Extend the existing metadata/paging proof with a reversed
  title order on a two-row page.
- The local affected-suite runner omitted `--no-fail-fast` despite the accepted
  full-Rust proof and CI using it. Both nextest and Cargo fallback now collect
  failures across targets.

## Verification in progress

The plan is `.lf/tmp/integrated-gate/plan.log`. The isolated runner removes all
inherited `LF_*`/`LOOPFLOW_*` authority, supplies a disposable Home/database, and
pins the source CLI. It uses the standard affected-suite commands, explicitly
including the app build and CLI smoke. Materialization uses a disposable copy
of tracked/untracked source instead of a raw Git worktree; development provenance
is explicit. No installed Home or configured provider is a test target.

The initial run is `.lf/tmp/integrated-gate/run.log`, with phase artifacts under
`.lf/tmp/gate/run-58800/`. Architecture coverage, migration-history validation
(56 shipped migrations unchanged), 310 Python cases, formatting and all-target
Clippy passed. The first materialization attempt stopped before Rust tests:
the private copy helper treated the Ghostty submodule directory as a file.
The helper now skips Gitlink directories, like a checkout with uninitialized
submodules; Rust does not use Ghostty. This is setup failure, not product proof.
The original failed receipt will remain failed. Additional results follow here.

Website: 78 passed, three skipped. The 291-case Swift run found one stale fixture
calling deleted `session import`; the other cases passed. The fixture now seeds
AgentSession/captured-event rows, as the maintained Rust observation proof does,
and retains actual native-client receipts. Its real-CLI automatic observation,
rescan, client exit and exact reader cancellation assertions are unchanged.
`swift-observation-repair.log` records all eight observation tests passing.
An earlier misspelled Swift filter selected zero tests (`swift-repair.log`),
which is not behavioral evidence. The multiplatform boundary check, Swift app
build, eight distinct nonempty fixture captures, CLI smoke and Xcode app/test-runner
build all passed. Fixture rendering is not configured Desktop acceptance.

The architecture reference now distinguishes completed owner/wire conversion,
Chapter fixtures and measured density from pending configured/installed acceptance.
Both website documentation checks and architecture coverage passed after that edit.

The full materialized Rust run is collecting every failure. Its first failure
was a prompt assertion still naming an ordinary “Run” after the canonical surface
changed to “contribution”; only that stale assertion is corrected. The original
full-run log remains unchanged. Focused repair evidence will follow.

Final checks must retain the acceptance boundary: no configured Codex/Claude/
OpenCode acceptance, live Linear rotation, installed migration, or promotion
was exercised. Existing fixture and native synthetic-Responses evidence remains
in `density/README.md` and the earlier working notes. Release conversion still
requires the exact candidate, a frozen database/filesystem rehearsal, quiesced
old writers, matching executable and backup, verified current Tasks/routes/native
identities, and reopening writers only after validation. The private converter's
live-sidecar reads are not an atomic backup or an approved deployment procedure.

Later Rust failures exposed four more stale setups. The lineage migration proof
now checks retained parent pointers at its historical frontier before finishing
the upgrade and asserting the old journal is gone. The headless retry proof
records an interrupted native outcome, asserts that release retains the input,
and explicitly retries before creating its replacement; stale-output rejection
and two-input history remain asserted. Command capture uses the current
`Command`/`sources` shape rather than demanding a retired decoder. The synthetic
published-PR fixture now includes its checkout event so selecting a parent can
still assert byte-identical prior history instead of testing missing-event repair.

The materialized full run finished: **2,004 passed, seven failed, 17 skipped**
(513.7 seconds test time; 93 seconds compilation). The final two failures came
from checkout/saved-sync fixtures trying to set `parent_pr_id` through the generic
PR writer, which deliberately no longer changes parentage. Both now call the
explicit stacking transaction. The saved-sync failure unexpectedly launched a
real conflict agent in its disposable repository; the retained output reports
no push. This was an unintended provider invocation, not configured acceptance;
native credential reads/refresh effects were not audited. Failing provider stubs
now prevent an erroneous fixture path from reaching real Codex/Claude/OpenCode.
The test's real Git parent selection, merge ancestry and repeated-sync assertions
remain. No installed-store mutation was requested by the gate.

## Completed local proof

`rust-repairs.log` records six passing focused materialized checks and two
published-parent fixture refusals. After adding synthetic publication evidence,
`stack-repairs.log` records those final two cases passing in the assigned source
checkout (no configured provider). This covers all seven original Rust failures
and the second consumer of the changed publication fixture. The full run and
original gate receipt remain failed; they are not relabeled as a clean final-tree
run. Required hosted CI must execute on the landing candidate.

Final all-target Clippy passed (`clippy-complete.log`), as did formatting, Ruff,
architecture coverage, migration-history validation and diff checks. No production
change followed the passing materialized Session paging regression. All later
Rust changes repair tests; the Swift fixture repair passed eight transport tests.

Review retained one runtime fix (cursor ordering), repaired current-model fixtures,
and aligned the local runner with CI's no-fail-fast behavior. No extra execution
owner, compatibility decoder, migration rewrite or restart API was added.

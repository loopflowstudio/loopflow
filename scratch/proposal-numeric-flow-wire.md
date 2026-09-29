# Proposal: captured numeric Flow node wire

2026-09-29 · LOO-298 · Prepared for Jack Heart. **UNAPPLIED, UNCOMPILED, UNTESTED.**
Main remains the executable writer and build owner. This contribution changes only
this artifact in tracked space; private snapshots, scripts and receipts are under
`.lf/tmp/numeric-flow-wire-proposal/`. No Git mutation, PM write, provider probe,
installed-Home access, build or product test ran.

## Ownership and exact scope

The public graph's `key`, `returns_to`, cursor `current`/`completed`, return
`decider`, and Session membership `node` become `u32`/`UInt32`. A key is the
captured preorder index, including **every** XOR alternative in sorted-name order.
It is local to one FlowSession. The existing optional `id` remains the authored
occurrence name, independent of numeric identity. No runtime cursor, repeat map,
Task pointer, Session lifecycle, native retry or execution authority changes.

The graph assigns preorder IDs while constructing its existing nested shape.
Cursor projection walks that same graph; `PinnedTaskFlow` constructs it once,
then reuses it for cursor and return projection. `QueuedInvocation::node_id`
remains the storage identity algorithm. The new nested fixture compares every
reachable captured cursor against that algorithm, not a second set of guessed
numbers. `node_at` now looks up the stored numeric key instead of decrementing a
counter to translate it into a structural path. Its import/classification callers
still need numeric lookup, so the method remains.

Swift keeps a selection as (invocation ID, numeric node). `FlowNode.contains`
walks existing child paths for state, selected-root scrolling and highlighting;
path details likewise inspect actual descendants. All prefix inference is removed.
Breadcrumb navigation still checks the exact invocation and refuses past or
unknown membership before selecting a current diagram. Pane ownership stays keyed
by Session. The unused `FlowNode: Identifiable` conformance and string `identifier`
are removed: their optional authored name was unsuitable as numeric graph identity;
repository callers already use explicit graph keys. This and the numeric field/
`project_cursor` signature changes are source/wire breaks without compatibility
defaults. External API consumers were not inspected.

One existing importer compared legacy manifest paths with display keys. Its
conversion is now private to `session_import::captured_node`: it walks the saved
graph using the legacy runtime path convention and returns the numeric key.
Historical immutable `RunFlowStep.node` strings and runtime `ExecutionCursor`
paths remain unchanged, as requested. This is retained import evidence handling,
not a second public wire decoder or permission to delete old captures. Unknown
node/iteration stays null; authored labels never infer missing location.

The patch includes all discovered Rust/Swift consumers, shared graph fixtures,
the inline Session UI fixture, and two current documentation statements. It adds
two explicit fixture graphs: one nested capture (preorder 0…9), and the captures
that ground the current/earlier/past Session membership matrix. Rust constructs
these graphs from concrete steps and checks fixture equality. Swift checks the
same IDs and graph relationships. Existing mounted selection/pane/draft assertions
are retained with the new IDs. Legacy import test inputs intentionally retain
string paths; current public assertions require numbers and reject a string cursor.

No product decision is needed for these conversions. Selecting a new decision
interface, paging contract, lifecycle or native retry behavior remains outside
this proposal.

## Review findings and boundaries

- A root index after an XOR is not its captured ID. Existing fixture root `7`
  follows XOR `6` and child `6/fix/0`, so it becomes **8**, while that child becomes
  **7**. Every graph fixture is traversed independently; only references in its
  own graph are rewritten.
- The older Session membership fixture had no accompanying graph. The added
  historical capture explicitly establishes `4/fix/1` as **6** and its later root
  `7` (demo) as **9**. A Rust construction check prevents those numbers becoming
  unsupported literals. Its other invocation IDs have separate captures; a missing
  older capture retains null location.
- Prefix removal must cover outer running state, diagram scroll target, selected
  chip and selected XOR path. The patch covers all four and retains exact nested
  detail lookup. Root and nested loops keep separate counts even when authored
  IDs repeat at different levels. Settled repeat storage is unchanged.
- Avoid duplicate graph construction: cursor projection consumes the graph already
  produced for the DTO. No additional capture hydration or persistence is added.
- The inline UI fixture still had a string node and obsolete scalar iteration.
  It now carries numeric review node 1 and `[[]]`, matching its two-step task-design
  capture. This is fixture reconciliation, not a new nullable-field default.

No behavioral pass is claimed. New nested/capture/import checks, current DTOs,
Task/taskless public reads, mounted terminal selection, Swift compilation and
all-target Clippy remain unproven on the proposal. Installed/configured-provider
or Desktop acceptance, native decision retry, dense discovery, complete Run wire
removal, historical migration and Chapter acceptance are not established here.
No SQL changes are proposed; a migration matrix is not required merely for this
wire cut. Main must retain the existing broader Task obligations.

## Patch verification and source drift

Snapshot head: `858444d9175248f0bc5987fc2f3b1a1786365a42`. Closing head: `858444d9175248f0bc5987fc2f3b1a1786365a42`.
Closing check: `2026-09-29T14:19:24.860003+00:00`.

Private Rust files passed `rustfmt --edition 2021 --config skip_children=true`
and its subsequent `--check`. This parses/formats Rust only; it does **not** type
check or run tests. JSON was parsed while converting fixture fields; this is not
a product decoder pass. Swift was source-reviewed, not compiled or executed.

The text validator independently parses every serialized hunk header, counts old
and new lines, replays the patch against saved original bytes **in memory**, and
compares the result byte-for-byte with the private proposal. **93 hunks
across 29 files** pass. Every nonempty old hunk context occurs exactly
once in both the original and closing shared source; new paths were absent.
Context is widened where repeated fixture graphs would otherwise be ambiguous.
Nothing was applied to the shared tree. Recheck these predicates at integration.

Main's concurrent changes were observed in:

- `rust/loopflow/src/run_record.rs`: initial `0cc4ccd50caf31849ca340c27e4fb449f84205a97f648438ef27a351b63b0182` → closing `b1a1d428239180faee1b4e84f79b1cfec682c5585208fd654381eaa797ba5515`.
- `rust/loopflow/src/journal/mod.rs`: initial `51bed76ade15a3c00927ffb4ea6c25a9131f62feb566f08044b837308c402767` → closing `8a0a22d0673e07dca2ea9575939560e71d2e74b3bcb3423cbf71ea195168fd45`.
- `rust/loopflow/src/bin/lf.rs`: initial `3c2fa887fb7f6eab27ee6ec05e258b2d1702ad9419148e012914aee57a600233` → closing `a7406c60fd12f75157da1a93f10c08b90bae1503d6a0e61127f4e6b69446933f`.
- `docs/architecture-reference.md`: initial `4cffbdd4c1885e055ae0422947818d028917dc7cfcd7a1fa5da62e115e7ba01c` → closing `262aae3ba1ead935402be3b388b0cd4cc76eed1542263ab2bf58d9987033527c`.

The first three files receive no proposed hunks. The architecture reference's
numeric-wire row still matches uniquely; apply that hunk, never the private
full file, to retain main's unrelated changes. All other snapshotted paths were
unchanged at closing, including the one-line Session CLI fixture hunk. Additional
later initial snapshots for `SessionFixture.swift` and `docs/lf.md` are timestamped
in `initial.json`; they were not treated as source drift. No claim is made about
uninspected concurrent files.

Patch SHA-256: `897ed66fc09cc37ed15a33b734329aafb22e8f0fe727952f6dee48b5acbd2ca2`; 117,929 UTF-8 bytes, trailing
newline included. The patch is also saved as
`.lf/tmp/numeric-flow-wire-proposal/numeric-flow-wire.patch`. The complete
validation receipt is `validation.json`; fixture old-key→captured-ID mappings
are in `fixture-node-maps.json`. These are proposal receipts, not build evidence.

## Production change measurement

**+187 / −196 = net -9.**
Measured against this contribution's saved source, not main or the whole branch.
Use line-based SequenceMatcher with autojunk disabled. Exclude integration/Swift
tests, new/shared JSON fixtures, Markdown and `SessionFixture.swift`. For Rust
source, remove the trailing `#[cfg(test)] mod …` body (not the first test-only
import or inline hook); no changed production hunk lies in the remaining test-only
helpers. Counts include comments/blank lines within the measured production diff.

| Source | Added | Removed |
| --- | ---: | ---: |
| `rust/loopflow/src/engine/flow_graph.rs` | +125 | −148 |
| `rust/loopflow/src/ops/human_session.rs` | +7 | −10 |
| `rust/loopflow/src/ops/session_import.rs` | +19 | −8 |
| `rust/loopflow/src/ops/task_flow.rs` | +5 | −4 |
| `swift/Loopflow/Models/SessionRecord.swift` | +2 | −2 |
| `swift/Loopflow/Models/TaskFlow.swift` | +15 | −11 |
| `swift/LoopflowMac/Views/TaskFlowView.swift` | +11 | −12 |
| `swift/LoopflowMac/Views/WorkspaceBreadcrumbBar.swift` | +2 | −0 |
| `swift/LoopflowMac/WorkspaceProjection.swift` | +1 | −1 |

## Focused commands for main — not executed here

Apply **hunks only**, after checking source drift; never copy the private source
files over concurrent work. Keep the existing single build slot and disposable
proof environment. Suggested affected proof sequence:

```sh
uv run python scripts/resource_envelope.py
uv run python .lf/tmp/cut-i/run.py numeric-flow-wire-rust cargo nextest run -p loopflow --lib --no-fail-fast -E 'test(engine::flow_graph::tests::) | test(ops::task_flow::tests::) | test(flow_membership_fixtures_cover_every_projection) | test(captured_nested_membership) | test(stored_session_membership_resolves_nested) | test(membership_wire_ids_are_derived) | test(legacy_paths_resolve_in_the_saved_capture)'
uv run python .lf/tmp/cut-i/run.py numeric-flow-wire-cli cargo nextest run -p loopflow --test flow_tests --test session_cutover_tests --no-fail-fast -E 'test(bound_flows_keep_task_context) | test(a_task_flow_runs_on_its_row) | test(import_stores_each_old_session_once) | test(import_preserves_unopened_and_finished_review)'
uv run python .lf/tmp/cut-i/run.py numeric-flow-wire-dto cargo nextest run -p loopflow --test dto_fixtures --no-fail-fast
uv run python scripts/test_task_installation.py --test task_flow_read_pins_topology_counts_both_returns_and_rejects_a_bad_restart
uv run python .lf/tmp/cut-i/run.py numeric-flow-wire-swift swift test --package-path swift --no-parallel -Xswiftc -gnone --filter 'TaskFlowTests|DTOFixtureTests/sessionFlowMembershipFixture'
uv run python .lf/tmp/cut-i/run.py numeric-flow-wire-terminal swift test --package-path swift --no-parallel -Xswiftc -gnone --filter 'WorkspaceNavigationProofTests/namedSessionDrillDownRetainsTerminal|TaskFlowProofTests/flowControlsRetainTerminals|SessionChromeProofTests/paneStripTrioFollowsHoverAndFocus'
uv run python .lf/tmp/cut-i/run.py numeric-flow-wire-clippy cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

The managed topology case is already designated for the disposable OS harness;
ordinary Home overrides are insufficient. Require actual executed Swift Testing
results and the mounted assertions, not XCTest's initial zero count. No native
provider test is needed to interpret a store/fixture decoding pass as transport
proof. Preserve any known failure separately instead of widening this proposal.
After the two documentation hunks, refresh derived docs through the established
website commands (`uv run python dev.py sync-docs`, then
`uv run python ../scripts/render_architecture_html.py` from `website/`) and run the
existing generated-documentation checks. Generated output is not included or
claimed validated by this research artifact.

## Exact initial and closing fingerprints

SHA-256s include changed paths and captured dependency/context inputs. Missing new
fixture files have no initial fingerprint; their proposal hashes follow. Full
receipts retain the same data for automated comparison.

```text
rust/loopflow/src/engine/flow_graph.rs
  initial 49e39b4596cb533b2b190b254706bb6eb0a7c402e60d8696cf313939b79e65e4
  closing 49e39b4596cb533b2b190b254706bb6eb0a7c402e60d8696cf313939b79e65e4
rust/loopflow/src/engine/invocation.rs
  initial 84aa6d60b6a2a72fe04bbf2cfdd01bdff010f738fc2660a6845f4822c9c4e885
  closing 84aa6d60b6a2a72fe04bbf2cfdd01bdff010f738fc2660a6845f4822c9c4e885
rust/loopflow/src/engine/execution.rs
  initial 3aee133d544a5397bdfa5a7d2a377906cdd5ad2f020d2c761bc084a4c1bb0c82
  closing 3aee133d544a5397bdfa5a7d2a377906cdd5ad2f020d2c761bc084a4c1bb0c82
rust/loopflow/src/ops/task_flow.rs
  initial 78e79e69cf0aebed096e51053a4b240dc2b7fe7c7d71032661f8354a32c00629
  closing 78e79e69cf0aebed096e51053a4b240dc2b7fe7c7d71032661f8354a32c00629
rust/loopflow/src/ops/human_session.rs
  initial 7ecc1bd79987941bd7646f9e453696f7133e40180a198d2f2ec364850aab4bd2
  closing 7ecc1bd79987941bd7646f9e453696f7133e40180a198d2f2ec364850aab4bd2
rust/loopflow/src/ops/session_import.rs
  initial 112a4a8c07b71759e98aa621b9a9e33d1f715f4d66eb096cd5cbe41a16c96ee6
  closing 112a4a8c07b71759e98aa621b9a9e33d1f715f4d66eb096cd5cbe41a16c96ee6
rust/loopflow/src/run_record.rs
  initial 0cc4ccd50caf31849ca340c27e4fb449f84205a97f648438ef27a351b63b0182
  closing b1a1d428239180faee1b4e84f79b1cfec682c5585208fd654381eaa797ba5515
rust/loopflow/src/journal/mod.rs
  initial 51bed76ade15a3c00927ffb4ea6c25a9131f62feb566f08044b837308c402767
  closing 8a0a22d0673e07dca2ea9575939560e71d2e74b3bcb3423cbf71ea195168fd45
rust/loopflow/src/bin/lf.rs
  initial 3c2fa887fb7f6eab27ee6ec05e258b2d1702ad9419148e012914aee57a600233
  closing a7406c60fd12f75157da1a93f10c08b90bae1503d6a0e61127f4e6b69446933f
swift/Loopflow/Models/TaskFlow.swift
  initial 15651efdeb5bdb6b77602db7dcd9c23ec48ff015f99fd55df052f8dcddef4857
  closing 15651efdeb5bdb6b77602db7dcd9c23ec48ff015f99fd55df052f8dcddef4857
swift/Loopflow/Models/SessionRecord.swift
  initial 5f07fbdd52c279ab9450e7d86c86bb33885084fc37701c22205c4803cc7d1c38
  closing 5f07fbdd52c279ab9450e7d86c86bb33885084fc37701c22205c4803cc7d1c38
swift/LoopflowMac/Views/TaskFlowView.swift
  initial 96406551328923784ad6efeac19c082586ef6b9bf9a19ce52d11a3f5c067af1f
  closing 96406551328923784ad6efeac19c082586ef6b9bf9a19ce52d11a3f5c067af1f
swift/LoopflowMac/Views/WorkspaceBreadcrumbBar.swift
  initial 6ce3c0bebbfba2f47d0a174fd1446791785b9bcbd85d61cf88e9708fba322720
  closing 6ce3c0bebbfba2f47d0a174fd1446791785b9bcbd85d61cf88e9708fba322720
swift/LoopflowMac/WorkspaceProjection.swift
  initial bba5d050698eb9fea6793ec3041fb01697d093e1dc6d5f495d5778ee186764c3
  closing bba5d050698eb9fea6793ec3041fb01697d093e1dc6d5f495d5778ee186764c3
swift/LoopflowTests/TaskFlowProofTests.swift
  initial 617998206ddcff8c54dfd4257f8d1fe06cf010af851013c9cf0f8f3fa7259982
  closing 617998206ddcff8c54dfd4257f8d1fe06cf010af851013c9cf0f8f3fa7259982
swift/LoopflowTests/WorkspaceNavigationProofTests.swift
  initial 09e8d5bda2ebdf6e2cdba243b8b9dcccae58437982cdfc36bbf34f90366e21c2
  closing 09e8d5bda2ebdf6e2cdba243b8b9dcccae58437982cdfc36bbf34f90366e21c2
swift/LoopflowTests/DTOFixtureTests.swift
  initial 98d74c15a4c93c82ad63ea04db6643087280895cf715eb29d6d884c2f3c49a75
  closing 98d74c15a4c93c82ad63ea04db6643087280895cf715eb29d6d884c2f3c49a75
swift/LoopflowTests/SessionChromeProofTests.swift
  initial 5acceb713c903d82c97c74ada2562d1f74a1217b3408330a0dc686fa64aa3e6a
  closing 5acceb713c903d82c97c74ada2562d1f74a1217b3408330a0dc686fa64aa3e6a
rust/loopflow/tests/flow_tests.rs
  initial 041143cd46ce512becdf88b575bd77919d3851c3ca195592f234d3146541b976
  closing 041143cd46ce512becdf88b575bd77919d3851c3ca195592f234d3146541b976
rust/loopflow/tests/session_cutover_tests.rs
  initial 36ec29c3bf3f8304cd99d3d6b9ea4e5814f751f1d6b9ca6a298e2f41f026fdb2
  closing 36ec29c3bf3f8304cd99d3d6b9ea4e5814f751f1d6b9ca6a298e2f41f026fdb2
rust/loopflow/tests/dto_fixtures.rs
  initial cb1071dd397022da53ba56a4152fefaf49efd1720becb1d814a3b24f5bd90496
  closing cb1071dd397022da53ba56a4152fefaf49efd1720becb1d814a3b24f5bd90496
tests/fixtures/dto/active_runs.json
  initial c94d62f749ef3cce1121bda8a08350f64f65ec30141931fe1178f9ea28322274
  closing c94d62f749ef3cce1121bda8a08350f64f65ec30141931fe1178f9ea28322274
tests/fixtures/dto/task_comments.json
  initial 6d9c65024674da2dfccf4614d5d8e5081580b0d880cf6d5d534a5075673ea781
  closing 6d9c65024674da2dfccf4614d5d8e5081580b0d880cf6d5d534a5075673ea781
tests/fixtures/dto/task_flow_stalled.json
  initial 73dda70dd66b022dbba36b7685d63cb984cfafc35dbf73ebead07f2adfb56a26
  closing 73dda70dd66b022dbba36b7685d63cb984cfafc35dbf73ebead07f2adfb56a26
tests/fixtures/dto/task_execution.json
  initial db807796942e76e5472ecf7727bf7e104f0937a954a15c61db7dd0e547e309e4
  closing db807796942e76e5472ecf7727bf7e104f0937a954a15c61db7dd0e547e309e4
tests/fixtures/dto/session.json
  initial a777114457ee000a2e3935bcfdd141cfa26337708251298f1752034490dc2875
  closing a777114457ee000a2e3935bcfdd141cfa26337708251298f1752034490dc2875
tests/fixtures/dto/sessions.json
  initial 88a506b1344ee94d06e3f394457338a5b8c5b990f478cd5314db094df49714d1
  closing 88a506b1344ee94d06e3f394457338a5b8c5b990f478cd5314db094df49714d1
tests/fixtures/dto/roadmap_snapshot.json
  initial 7eeaf1d0f56225eccc506a5f6ee253cca5574ea6fdbd4ed9fbaafe9234602b38
  closing 7eeaf1d0f56225eccc506a5f6ee253cca5574ea6fdbd4ed9fbaafe9234602b38
tests/fixtures/dto/task_execution_stalled.json
  initial 1add8d1d8d098e373cf39e1dfa97ceae0687884ab84a2a60050a8286dd0fa315
  closing 1add8d1d8d098e373cf39e1dfa97ceae0687884ab84a2a60050a8286dd0fa315
tests/fixtures/dto/wave_detail.json
  initial 4ada1cec32d3eb7c95dee4ff1dc64f66587806fc323eabcf340bce27d1ca684f
  closing 4ada1cec32d3eb7c95dee4ff1dc64f66587806fc323eabcf340bce27d1ca684f
tests/fixtures/dto/child_control_activity.json
  initial 0b72ef206880f7396048a7713b1719c791ee99d9f8ee9f29482d04608311b45b
  closing 0b72ef206880f7396048a7713b1719c791ee99d9f8ee9f29482d04608311b45b
tests/fixtures/dto/work_statuses.json
  initial 8d277e8abe85d6beca2ff378982c0d288230135cf69ca6850cac8decabd43f99
  closing 8d277e8abe85d6beca2ff378982c0d288230135cf69ca6850cac8decabd43f99
tests/fixtures/dto/session_actions.json
  initial adc70db1db9ad5c0c89ef9167111201106162178bd0e3259041c71604c77e637
  closing adc70db1db9ad5c0c89ef9167111201106162178bd0e3259041c71604c77e637
tests/fixtures/dto/metric_portfolio.json
  initial 6137dd00fd51ec1a06f69a683ada5a661352e9bc8f7725306485e00d72d794a5
  closing 6137dd00fd51ec1a06f69a683ada5a661352e9bc8f7725306485e00d72d794a5
tests/fixtures/dto/activity_snapshot.json
  initial 5f49a566c5b0ba1a0bfa53f3bc1d04848b226a10d2a752de6a2035944986834d
  closing 5f49a566c5b0ba1a0bfa53f3bc1d04848b226a10d2a752de6a2035944986834d
tests/fixtures/dto/pm_show.json
  initial 4090708e88c16748e4cb1f592b0d7b90d3047e15cc37974a745077e0e81f2b89
  closing 4090708e88c16748e4cb1f592b0d7b90d3047e15cc37974a745077e0e81f2b89
tests/fixtures/dto/task_condition_states.json
  initial a21c24b356305a14a17941823e08436cc8b0ad9b056cb23bdb975682e5660c24
  closing a21c24b356305a14a17941823e08436cc8b0ad9b056cb23bdb975682e5660c24
tests/fixtures/dto/task_files.json
  initial 6639a1d2cfa1f070e1f39639a8b6665750eb37dd5d01d976e8e1628568c317b6
  closing 6639a1d2cfa1f070e1f39639a8b6665750eb37dd5d01d976e8e1628568c317b6
tests/fixtures/dto/work_activity_snapshot.json
  initial 2de67abdd0aa337094c58932ba88fdadeaed932cca47bb3a9e913d08cf4606e1
  closing 2de67abdd0aa337094c58932ba88fdadeaed932cca47bb3a9e913d08cf4606e1
tests/fixtures/dto/flow_catalog.json
  initial c727a27ab0ac4e7c851d96659506b33a265ad730604742b19248a7de5355f7a1
  closing c727a27ab0ac4e7c851d96659506b33a265ad730604742b19248a7de5355f7a1
tests/fixtures/dto/session_history.json
  initial 66bf1f376bc67575e14507e0e330d5fac95ee71f2e2b12d9ab4932b9896d4043
  closing 66bf1f376bc67575e14507e0e330d5fac95ee71f2e2b12d9ab4932b9896d4043
tests/fixtures/dto/auth_status.json
  initial 24dabd335029330f5243d9773fa37393ac8845940d1b6999930c886be5a8389a
  closing 24dabd335029330f5243d9773fa37393ac8845940d1b6999930c886be5a8389a
tests/fixtures/dto/session_memberships.json
  initial f33af6edd534f3ebd5bd38d02717f80d81d732d8792a070974808b738bf22fb3
  closing f33af6edd534f3ebd5bd38d02717f80d81d732d8792a070974808b738bf22fb3
tests/fixtures/dto/task_flow.json
  initial 32edc1864922efd861e7fc71ed7fb40af29543d7d7cca395306cbaf0bc1dbbd3
  closing 32edc1864922efd861e7fc71ed7fb40af29543d7d7cca395306cbaf0bc1dbbd3
tests/fixtures/dto/auth_routes.json
  initial 42a887756a9d0594dbb26abb7c3f666e17b3bdb0e8001fc02dd51d6195128264
  closing 42a887756a9d0594dbb26abb7c3f666e17b3bdb0e8001fc02dd51d6195128264
AGENTS.md
  initial 0baa37feb013dbc5ef2303b9975c795ade0e2183843e1e66df4b824698eb71fc
  closing 0baa37feb013dbc5ef2303b9975c795ade0e2183843e1e66df4b824698eb71fc
README.md
  initial 972ce54387a26b2f483831b649319b7c23fa40aed16da40669bb510f1c70a2a4
  closing 972ce54387a26b2f483831b649319b7c23fa40aed16da40669bb510f1c70a2a4
scratch/data-model-one-table-per.md
  initial 49afb1c57c40f1720718136191ad2682945ceec5c19f21fcfde52c5d4ff66caa
  closing 49afb1c57c40f1720718136191ad2682945ceec5c19f21fcfde52c5d4ff66caa
scratch/remaining-work.md
  initial 1bfdf463cb265acc138237920141cb3b6613b7211e06d8d1ce0ba5e8628f804c
  closing 1bfdf463cb265acc138237920141cb3b6613b7211e06d8d1ce0ba5e8628f804c
scratch/research-session-wire.md
  initial 37ebf65ef6c1da496dd796e233b0a1c3eb45662076272b95b77fe46d897d9d9b
  closing 37ebf65ef6c1da496dd796e233b0a1c3eb45662076272b95b77fe46d897d9d9b
scratch/questions.md
  initial 47e04a26b88c081b8d2d479879d54d8c413018353c744824c9329a3df327a1f3
  closing 47e04a26b88c081b8d2d479879d54d8c413018353c744824c9329a3df327a1f3
docs/architecture/execution.md
  initial 64436292cf39d1c82227a1746e82d9adeba70366b013531d92380b30cda673a3
  closing 64436292cf39d1c82227a1746e82d9adeba70366b013531d92380b30cda673a3
docs/architecture-reference.md
  initial 4cffbdd4c1885e055ae0422947818d028917dc7cfcd7a1fa5da62e115e7ba01c
  closing 262aae3ba1ead935402be3b388b0cd4cc76eed1542263ab2bf58d9987033527c
wave/infrastructure/MEMORY.md
  initial ae55194066cf9a94437afadb2541fbee025caace298a5e472b7a94e2e0e9bc26
  closing ae55194066cf9a94437afadb2541fbee025caace298a5e472b7a94e2e0e9bc26
swift/LoopflowMac/SessionFixture.swift
  initial 0635409f21642f7f13c6097053a25bf542b6e79e8d40ad0ada103315b9aa448f
  closing 0635409f21642f7f13c6097053a25bf542b6e79e8d40ad0ada103315b9aa448f
docs/lf.md
  initial 698622c4103476793c976f310a1c78da5ddfd6d22edbd4093dfeeab0b3d1748d
  closing 698622c4103476793c976f310a1c78da5ddfd6d22edbd4093dfeeab0b3d1748d
```

Proposed file hashes (private bytes; **not** the integrated tree):

```text
217f4ac14a876b9381e1c7db83b5a6edbd17be715bd01e6ded4e9f33fc2fe3bb  docs/architecture-reference.md
ba3da34ed78d249f5d8cc8656d63421076cb1e8bf0dca73b08f2204fa16768ed  docs/lf.md
5d28df3048e4753a447e49cef3a25fb29cad3fb3786f48c468791d7b09028f65  rust/loopflow/src/engine/flow_graph.rs
900a859db67f4c36aac1cf0355fae40569421cf72b71b8045ad71f996fc54c9b  rust/loopflow/src/ops/human_session.rs
277cc392f20a006e8d934147aa478208062f8f85d7f5e4b6cbf400b4ce7f71e5  rust/loopflow/src/ops/session_import.rs
e54f87514089a4718205794de1e1436a4a49b71de8d1bd612309dd597e75b892  rust/loopflow/src/ops/task_flow.rs
4c94f9ef5408fd21d241d9c3d26ed56425feaf0ef5cd7a9ada8f494f2467f896  rust/loopflow/tests/flow_tests.rs
25e823ff2d72207bb47fab17d68d40a30f740830ab8f0fc41b05b81c73c85dc8  rust/loopflow/tests/session_cutover_tests.rs
41bb59573d6c19b5b8923798d8dc23f7f1814a6b175e89f77d4c55d54d74b270  swift/Loopflow/Models/SessionRecord.swift
cf86226f5250a5c3c4f85be819cf50068c343cbf222d1ec5898b0aa49efb8aaa  swift/Loopflow/Models/TaskFlow.swift
a9839fc90f86472c839aa724d06d5f3910aab2fe2114cedd8662287855ef614d  swift/LoopflowMac/SessionFixture.swift
ded4ae2ddc0690a0078c30162fd8720229b3984ae66d41170241ff41f36d1487  swift/LoopflowMac/Views/TaskFlowView.swift
9fced50f0f18e58759a1ac205a0c25b9fb9736e65ada3a8f337ebf75bb6d5247  swift/LoopflowMac/Views/WorkspaceBreadcrumbBar.swift
8816030136f96e0b11663dfaa4b877074b14f9f3ed1df86c97f8b564181b20d0  swift/LoopflowMac/WorkspaceProjection.swift
a5afb48e3249338b3226ddb6c86b6b7f0b8d939da290a709788c838c5f4807c7  swift/LoopflowTests/DTOFixtureTests.swift
ae0741e654dca99a1d2b42efb3d856ca6d44d0371ec1aebcf6fe0e2201f349c6  swift/LoopflowTests/SessionChromeProofTests.swift
2a87e42246a82bc89362c353c72272b437da1a42a9e54398d06d13b18d1b45d9  swift/LoopflowTests/TaskFlowProofTests.swift
b25dc49c8f791c2f7972d14c5a0d048a09a16b1340d4ddb4b198a7c1449031a0  swift/LoopflowTests/WorkspaceNavigationProofTests.swift
4b3b831da8e9c645ec8013b7f727b568058e886bb1d415c5aac369228467eb6f  tests/fixtures/dto/flow_catalog.json
3e4326bc99d4ce12758132ec7348d4974bacfe28febeba60804b2ae6a44c22e7  tests/fixtures/dto/flow_numeric_nested.json
ed5d2639e67f47fbf646c4c727e38f7569507f09c2b772ac2a4f818d8161b8c4  tests/fixtures/dto/roadmap_snapshot.json
4fe4ae8ad986d82b3a37e35b159c01cf35320213743a3ab1621d9cdc904301be  tests/fixtures/dto/session.json
8f02fcaebc439cbec2fcd88621d34f63d185f3f0779a1bf673d0d2b235297f26  tests/fixtures/dto/session_membership_graphs.json
1b6fa90d2939e3308a65f2e1012b1fed75c6c62df61f27a78b032ca20c59d545  tests/fixtures/dto/session_memberships.json
1622c26693dfdf538dadd70e03469f16c137f712bcda86b12c1c829626106ede  tests/fixtures/dto/sessions.json
8ca2d1417455d5da7ea7b9565c8a10cf9973f202a79f85b20b84d93746d04c70  tests/fixtures/dto/task_condition_states.json
ee06ea5736b79ddf1c59b2097428b67286fa825da3fd10b574c92cd680e81794  tests/fixtures/dto/task_flow.json
d8a666e08b22801c6a0527d8701c44fedbfd7b0adb9f6b865f85f39ad0902b30  tests/fixtures/dto/task_flow_stalled.json
53523e09d007d9d833e36a22440714d48c644cac95efcdc0dd4d27ef5e542ea5  tests/fixtures/dto/wave_detail.json
```

## Complete unapplied unified patch

Read `.lf/tmp/numeric-flow-wire-proposal/numeric-flow-wire.patch` on disk.
SHA-256: `897ed66fc09cc37ed15a33b734329aafb22e8f0fe727952f6dee48b5acbd2ca2`.
The full original proposal is preserved at
`.lf/tmp/numeric-flow-wire-proposal/proposal-with-inline-patch.md.txt`
(SHA-256 `4834bdd5cae8d90407d6365003d026282d20c1bdc6f4258c9725dfcdc096c700`).

Supervisor removed only the duplicate inline patch from launch context after
Codex rejected Run `run_a12ee9b58fa749fc93d7caedb4df73aa` at its 1,048,576-character
input limit. The patch, review findings, source fingerprints and proof obligations
are unchanged. Main must read and review the actual patch before applying it.

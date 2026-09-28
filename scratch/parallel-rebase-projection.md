# Rebase projection audit

2026-09-28 · LOO-298 · Bounded read-only contribution for Jack Heart's
authorized parallel review. Compared `1012ae4fba1abadc92e0eee042b9698952e3e9b6`
with `ce97003cfa37e26baaa0f5a080d68ae4fbc17178`, then inspected concurrent
dirty repairs. Read the working design and `scratch/reviews/parallel-import-review.md`.
No tests, builds, provider calls, Home access, Git mutations or executable edits.
This note is source evidence, not a passing verification result.

## Prioritized finding

**P1 — Wave enablement survived its upstream owner; repaired in dirty source,
verification still due.** At `ce97003cf`,
`rust/loopflow/src/lf/commands/waves.rs:801` reads `placement.enabled`, but
`rust/loopflow/src/durable.rs:122` defines Placement with only Work, Home and
placement time. The Wave DTO, Swift `RegistryQuery.swift:306`/`Wave.swift`,
and `WaveLens.swift:72` still expose enablement and can describe a Wave as
“Disabled on this Home.” This is incomplete reconciliation of upstream's
removal, not part of the planned AgentSession/FlowSession conversion.

The inspected dirty edits remove the Rust access/DTO/display field, Swift
field/lens branch, dependent fixture values and assertions. Preserve that
coordinated repair. **Correction to the interim audit hypothesis:** committed
`wave_detail.json` and `roadmap_snapshot.json` still contained `enabled`; there
was no committed Rust/Swift fixture decode disagreement. The concrete defect
was the removed placement member and obsolete projection.

Smallest future proof: compile the affected Rust projection, run its existing
DTO fixture checks and Swift Wave/RegistryQuery/DTO/WaveLens checks on the final
combined bytes. Decode a Wave with required `home` and without `enabled`; its
active Task count must remain visible. Main owns that build slot. No additional
unrepaired rebase-specific DTO or Desktop identity defect was established here.

## Reconciled paths to retain

- `RegistryQuery.swift:45,63,127`, `PodiumModel.swift:564`, and
  `MacLocalWaveAgentLauncher.swift:82` now use `wave list/status`, `task run`
  without `--flow` for continuation, and `task checkout --json`. The Rust
  dispatch matches these commands; checkout still prints Task's top-level
  worktree. `flow list --json` remains explicitly supported in
  `rust/loopflow/src/bin/lf.rs`; replacing it with bare `catalog` would lose
  its typed JSON contract. `session open` remains the alias of `connect`.
- `SessionRecord.swift` and `ops/human_session.rs:195` agree on interactive
  visibility, typed Work/Wave, title provenance, optional Flow node/iterations,
  actions and terminal IDs. `session.rs:10` and `SessionEvent.swift:4` retain
  identical native-history fields, including nullable generation/Exec/Task/Wave
  attribution and arbitrary payload. `session_history.json` and both language
  fixture readers survive. These Session/history declarations did not change
  across this rebase.
- `TaskFlow.swift` retains captured graph, current/completed positions, return
  counts, iteration tuples and restart policy matching `ops/task_flow.rs`.
  Project status/required Flow from H7 also remain. Catalog fixture changes to
  realign describe today's template; they do not replace a saved capture.
- `RegistryQuery.swift:187` retains one `--limit 0` Session inventory, avoiding
  separate offset pages. `WorkspaceProjection.swift` is unchanged; its outline
  keys Sessions by `session.id`. `Views/SessionsView.swift:425,453` reconciles
  and opens panes using that stable ID, not title or current Run. Smallest
  future integrated proof: refresh 101 conversations while renaming one and
  replacing its current Run; retain the selected Session and its terminal
  surface. The existing inventory fixture alone does not prove pane retention.

## Existing conversion gaps, not rebase regressions

The accepted physical-owner/history work remains pending: public Run references,
Flow exact-success references, Exec history presentation, and Desktop history
consumption cannot be marked complete from retained DTOs. Swift has a SessionEvent
decoder but RegistryQuery has no history reader. Workspace grouping still joins
Task Sessions through visible roadmap runtime IDs; a bound Session whose Task is
absent remains reachable as unmatched, with no Task breadcrumb. These existed at
`1012ae4fb`; retain them with the scheduled conversion and import obligations,
not as new redesign work. The future done/landed-Task proof should preserve the
Session ID, typed attribution and pane even when current planning omits the Task.

## Source fingerprint

HEAD remained `ce97003cfa37e26baaa0f5a080d68ae4fbc17178` at the final inspection.
Main was concurrently editing; hashes identify inspected bytes, not later work.
SHA-256:

```text
d57dfc4ddcf8c5be77643e877953d500da9a56502fea40558fd802e98ed9aa24  swift/Loopflow/Services/RegistryQuery.swift
5f07fbdd52c279ab9450e7d86c86bb33885084fc37701c22205c4803cc7d1c38  swift/Loopflow/Models/SessionRecord.swift
dc96b2186b1bf8403e98c082eddd0451f805a6a3a5f0307ac95f8fcf717831c0  swift/Loopflow/Models/SessionEvent.swift
15651efdeb5bdb6b77602db7dcd9c23ec48ff015f99fd55df052f8dcddef4857  swift/Loopflow/Models/TaskFlow.swift
326c788f78a6dcc720566f2c253995154988f270fa096f1a1dfcc5dbe9787942  swift/LoopflowMac/WorkspaceProjection.swift
378b4a94dc3c978d31c6712ceafda2a58aa0c3fe415a2af5e698bb8f2869c679  swift/LoopflowMac/Views/SessionsView.swift
5664ea7107f0fd0c5bde3f0a8d5db538f0cc2d7d12eee564a6c4f8b5aa32d79a  rust/loopflow/src/ops/human_session.rs
2a555be4744e5915b339a009fbb6e86d092fb7b60f9d4111fda3a47666287dab  rust/loopflow/src/session.rs
3996dc23b789f43db74a4c723a915727900d58113615cf0e0492f1476f85aec1  rust/loopflow/src/lf/commands/waves.rs
4ada1cec32d3eb7c95dee4ff1dc64f66587806fc323eabcf340bce27d1ca684f  tests/fixtures/dto/wave_detail.json
630ed679494af5e36947c62082115e0d860e3573d4d30091865931181a87892a  tests/fixtures/dto/session_history.json
```

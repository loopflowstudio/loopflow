# XOR definition pinning

2026-09-25. Bounded contribution to LOO-295. Read `concept-review.md` first.
Edited only `rust/loopflow/src/engine/flow.rs` and this note. Other files and
the concurrent removal of repeat limits in flow.rs belong to main.

## Captured definition

`ConcreteXor` now contains `router: Skill` and
`paths: HashMap<String, ConcretePath>`. Each `ConcretePath` contains its
description and `Vec<ConcreteStep>`. Authored `XorDef` and `XorPath` are unchanged.

Expansion captures every path recursively before execution, including unchosen
paths, their routers, skill bodies and launch metadata. The generated router is
named `xor-route`, defaults to `claude:sonnet`, and teaches `lf flow route PATH`.
The routing suffix reads captured descriptions and teaches the same command.
`read_xor_verdict`, the shared file protocol, and public `load_xor_path_items`
are removed. Branch expansion is private.

Flow references through XOR retain the parent chain and depth rather than
restarting expansion. Explicit and implicit recursive references are rejected;
the existing depth limit spans those references. A Flow may still invoke a real
same-named skill, as builtin `launch-plan` does. Skill resolution now returns an
error for missing content instead of leaving an unresolved name in the captured
tree, including ordinary steps. Main should account for this stricter expansion
behavior in fixtures that previously used nonexistent named skills.

Occurrence identity and human-gate validation traverse the captured tree.
Backward edges are validated in each branch's own body; a child cannot target
an outer occurrence. The existing global occurrence-ID uniqueness rule remains.
Human-ID collection no longer reloads nested definitions.

Legacy unresolved XOR records fail decoding: the old optional router name cannot
decode as a captured Skill, and ConcretePath rejects authored `flow`/`skill`
fields, including when an old record contains an empty `steps` array. There is
no source-reloading migration or invented captured content. Main owns surfacing
that saved-state error and its recovery disposition.

## Focused proof

All **45 flow.rs tests passed**, including these changed/new tests:

- `xor_snapshot_preserves_all_routes_and_skills_after_source_deletion`
- `xor_expansion_rejects_cycles_and_excessive_depth`
- `xor_expansion_validates_nested_edges_and_human_ids`
- `xor_expansion_rejects_missing_content_in_unchosen_paths`
- `xor_legacy_unresolved_records_fail_without_loading_sources`
- `build_xor_routing_suffix_sorts_paths`
- `load_flow_expands_all_builtin_flows`

The snapshot test expands nested selected/unselected alternatives, changes skill
sources and observes changed bytes in a new expansion, then deletes all local
Flow/skill files and decodes the original snapshot unchanged. It checks router
and skill bodies, agent metadata, human IDs, empty branches, descriptions and
ancestry. Builtin coverage now checks captured content recursively.

To avoid main's concurrent consumer integration, verification used a temporary
Cargo harness with path imports of the real `flow.rs`, `builtins.rs`, and
`error.rs`. The generated builtin catalogs refer to the checkout's current
source files through `include_str!`. No parser or loader was replaced.

```sh
env -u LF_CONTROL_HOME -u LF_CONTROL_DB_PATH -u LF_HOME -u LF_DB_PATH \
  -u LF_RUN_CONTEXT -u LF_RUN_ID -u LF_RUN_DIR \
  cargo test --offline \
  --manifest-path /var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/loopflow-xor-proof-nfpck8gz/Cargo.toml \
  engine::flow::tests:: -- --nocapture
```

Harness `cargo clippy --offline --all-targets -- -D warnings` passed with the
same manifest. File-scoped rustfmt and diff whitespace checks passed.
The first compilation found one old builtin test still accessing `path.flow`;
it now inspects captured steps. The next test run caught same-named `launch-plan`
resolution; fixing that restored the full module pass.

## Review and integration boundary

One recursive expansion owns the snapshot; validation consumes it without a
second loader. Strict legacy decoding prevents an old empty branch from silently
discarding a saved Flow reference. No runtime abstraction, factory, provider,
Session or store was added.

Main owns all consumer updates, exact route authority and persistence, Task and
ordinary traversal, and runtime recovery. These tests prove captured definitions
and validation, not actual provider routing or exactly-once runtime execution.
No repository-wide test, commit, push, installation, Task or Session operation
was performed.

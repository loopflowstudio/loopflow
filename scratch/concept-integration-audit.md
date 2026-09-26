# Concept-review integration audit

2026-09-25. Bounded LOO-295 contribution. Read the human feedback in
concept-review.md, cursor-integration.md and protocol-review.md; reused the
recorded historical findings without repeating the history survey. HEAD at
final inspection: `0ef41c784`. Runtime edits in this shared checkout belong to
main and the cursor contribution.

## Finish line and changes

Canonical skills must preserve product simplification as a result in itself,
draft usage before implementation design, distinguish proposals from accepted
requirements, and give navigation solely to the deciding occurrence. Actual
catalog loading and Flow traversal distinguish integration from plausible
prompt wording. This review does not establish live provider judgment quality.

Changed only:

- `docs/authoring.md`: teach the attended and autonomous concept-review use,
  usage-first design and evidence handoff. Correct the XOR no-op description:
  an inline `steps:` body is executable too.
- `docs/lf.md`: replace the misleading finite task-design description with its
  actual human Advance/Iterate behavior; clarify that an edge repeats its own
  body, so an authored design revision may revisit initial design work.
- `docs/architecture/data.md`: document ordinary Flow position files, human
  Ask records, the shared cursor, Task transaction ownership, exact human
  authority, successful-Run candidate settlement and keyed Ask retention.
  Distinguish historical SQLite Ask tables from current human Ask Sessions.
- Canonical `task/skill/concept-review.md`: return review evidence without
  implying every finite Flow or standalone review needs a loop-decide step.
  Existing human corrections, preserved-behavior requirements and autonomous
  limits remain intact.
- This note.

The canonical loop-decide and unblock skills already satisfy the accepted
contract and remain unchanged. So do the selected builtin Flows: build, slice
and task-gate place concept-review directly after review-slice; pursue adds
loop-decide with the backward edge to implement, then human demo with its own
revision edge. Feature composes task-design and pursue. No pass limits remain
in these definitions. There is no reason to add decision steps to finite flows.

## Validation

**19 tests passed, 0 failed** in the real repository, using its generated
builtin catalog and actual loaders, expansion and Task transition code:

```sh
env -u LF_CONTROL_HOME -u LF_CONTROL_DB_PATH -u LF_HOME -u LF_DB_PATH \
  -u LF_RUN_CONTEXT -u LF_RUN_ID -u LF_RUN_DIR -u LF_FLOW_STEP \
  -u LF_HUMAN_SESSION cargo test -p loopflow --lib -- \
  engine::builtins::tests \
  engine::flow::tests::load_skill_finds_all_builtins \
  engine::flow::tests::load_flow_expands_all_builtin_flows \
  engine::flow::tests::authored_human_edges_keep_their_identity_and_revision_target \
  controller::task::planning_tests::feature_repeats_the_whole_slice_then_stops_for_delivery \
  ops::human_session::tests::ask_skill_and_question_survive_reopening_and_reach_the_launch_prompt
```

The feature fixture traverses ten passes through both reviews and loop-decide,
then human Iterate returns to implementation and permits another autonomous
Iterate. The Ask fixture exercises real JSON persistence, launch argument
parsing and canonical prompt preparation. Provider/PM effects are simulated;
no live human Session was opened. Existing builtin contract tests are included
as required by TESTING.md, not treated as behavioral proof by themselves.
No new prompt-substring assertions were added.

Read-only `uv run python` / PyYAML checks passed:

- Three canonical skill frontmatters have valid required metadata.
- All four direct builtin review-slice successors are concept-review.
- Pursue's parsed sequence, decision edge, human gate and revision edge match
  the intended contract, with only `from` on each backward edge.
- The three docs' YAML examples, fences, local links and heading anchors pass.

Scoped `git diff --check` passed. The test run initially waited on the shared
build lock, then compiled and completed successfully. `lf ps --json` was used
only to observe machine health; no process was signaled. After the test pass,
only prose wrapping and the historical-recovery qualification were adjusted.
No runtime change, new runtime test, full-suite or Clippy pass is claimed here.

## Review findings and handoff

The model now names one traversal owner and separate legitimate persistence
owners. A Session projection cannot grant another cursor's authority. Review
evidence is useful both with and without a following loop-decide. No lifecycle,
configuration or extra review pass was introduced.

Adversarial review retained these distinctions: product clarity can improve
without reducing code; unresolved review-slice failures remain failures; a
proposal cannot replace approved requirements; changed claims need renewed
behavior proof; Ask Complete returns evidence rather than Flow approval; no
meaningful progress calls for help rather than a counter-based stop.

Adjacent launcher skills outside the three selected skill bodies still abbreviate
the pursue sequence without loop-decide: `task/skill/advance.md`,
`task/skill/launch-plan.md` and `ops/skill/loopflow.md`. Their sequence summaries
should include the deciding step during main's broader prompt integration.
They were inspected but not edited in this bounded contribution.

Main is concurrently adding explicit saved-XOR recovery and removing duplicate
cursor bodies. The architecture text preserves the historical-evidence contract
without claiming that their in-progress verification has passed. Native
provider/Ask/human-gate handoff, combined runtime validation and release remain
main's obligations. The Wave/UI deletion inventory stays deferred as directed.

No edits to main's scratch/concept-review.md, runtime or personal skill exports;
no commit, push, install, Session mutation, orchestration or additional agents.

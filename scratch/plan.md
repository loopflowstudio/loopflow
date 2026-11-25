# LOO-1108: builtin Graphs end at deliver; the verify skill goes

## Outcome

`code` and `feature` end when `deliver` merges the PR. The Graph reaches `end`,
which already requests completion (`docs/work.md` "A Task whose Graph reaches
`end` requests completion automatically"; `quick` and `deliver`-ending repo
Graphs do this today). Nobody is asked to come back after the merge.

| | before | after |
|---|---|---|
| `code` line | plan build ☺ship-review deliver ☺verify | plan build ☺ship-review deliver |
| `code` sentence | You take part at ship-review and verify. | You take part at ship-review. |
| `code` in Start | 2 stops | 1 stop |
| `feature` sentence | You take part at explore, ship-review and verify. | You take part at explore and ship-review. |
| `feature` in Start | 3 stops | 2 stops |
| `lf verify` | runs the builtin | `skill or graph not found: verify` |

## Stacking

This branch is stacked on LOO-1104 (PR #2047, open): `lf sync LOO-1104` ran
before any edit, as two steers asked ("Stack this Task on LOO-1104 by running
`lf sync LOO-1104` first, before editing code.yaml, feature.yaml, or
verify.md."). #2047 rewrites `verify.md` and adds `quick.yaml`; it does not
touch `code.yaml` or `feature.yaml`. If #2047 merges first, `lf sync main`
detaches. Never `git merge` its branch by hand.

## The Graphs

```yaml
# code
description: Build and deliver. You review the change before it merges.
steps:
- graph: plan
- loop:
    - graph: build
    - interactive: ship-review     # you read the page; iterate goes back to build
- graph: deliver
```

`feature` is the same with `- interactive: explore` first and
`description: Explore, build and deliver. You choose what to build and review
the change before it merges.` Desktop's Start menu shows the first sentence
(`StartFlow.summary`, `swift/LoopflowMac/Views/BacklogView.swift`).

The outer loop goes with `verify`. It existed for verify's "fix forward"
iterate, and it cannot stay: a loop's last step must be a skill
(`rust/loopflow-execution/src/graph/mod.rs`, "a loop's last step decides
whether to run it again"), and `deliver` ends in `cmd: land --wait-and-fix`.

## Findings

- **Tasks already on `code`/`feature` keep `verify`.** A Task stores its Graph
  compiled, skill text included (`CapturedGraph` holds `ConcreteSkill.skill`;
  `take_up_task_graph` in `rust/loopflow-execution/src/ops/task.rs`).
  `.lf/qa/auto.md` records the same for the earlier `human-review` rename: "A
  Task left waiting there ships after the upgrade (the captured Graph carries
  its skill)". No migration. This Task's own run is one of them.
- **`lf task run` from `ship-review` now prints `merges:`.** With auto unset,
  the line follows `TaskRunSelection::reaches_end`, false until now because
  `verify` lay ahead. The merge itself never depended on it: only an explicit
  `auto off` waits at `land`. The line becomes true to what the run does.
- **Other repositories name the builtin.** On this Machine:
  `manabot` and `fantasia` have `- interactive: verify` in
  `.lf/graphs/{coverage,engine-performance,experiment-performance}.yaml` with
  no skill of their own; `etude` and `etude-sdk` have `.lf/skills/verify.md`
  with `extends: builtin`. After a release with this change, those Graphs fail
  a new take-up with "skill not found: verify" and the `extends: builtin`
  skill has nothing to extend. The repair is a hand edit there (drop the step,
  or write the skill out in full). No loopflow code migrates them.

## Choice made for review

Nothing replaces what `verify` did after merge: reading post-merge CI for the
merge SHA, checking claims against `main`, fix forward. Passed over: folding a
post-merge check into `deliver` after `land` (it would put a headless agent
step behind every merge, the cost the removal is cutting), and keeping the
skill as an unreferenced builtin (a builtin no Graph runs is dead prose). A
repository that wants the stop writes `- interactive: <its own skill>` after
`deliver`; the engine still supports an interactive step after a merging one.

## What goes

- `rust/loopflow-definitions/src/builtins/task/skill/verify.md`.
- `- interactive: verify` and the outer `loop:` in `code.yaml` and
  `feature.yaml`.
- `verify` in Rust tests of the builtins: `task_graph.rs` (interactive names,
  the "after the last interactive step" case moves to `ship-review`, the
  workflow line, the doc comment), `graph/compiled.rs` (steps and sentences).
- Tests that borrow the builtin as a stand-in skill get another one, behavior
  unchanged: `rust/loopflow/tests/work/task_graph_launch_tests.rs`
  (`verified.yaml`, `verifying.yaml`: an interactive step after a landing step
  and a loop decided by an interactive step, both still supported),
  `skills/catalog.rs` (#2047's `.lf/skills/verify.md` expecting `Replaces`
  needs a name that is still a builtin), `ops/task_execution.rs` (a step
  label).
- Swift: `Graph.swift` doc comment; sample data in `WorkflowLineTests`
  (five steps become four, tone arrays shrink), `BacklogViewTests` (`code`
  becomes 1 stop, "1 stop"), `AppBootstrapResourceTests` (sample word).
- Fixtures: `tests/fixtures/dto/graph_catalog.json` and
  `server_schema.json` descriptions. `skill_entries.json` has no `verify`
  entry; its only match is `debug`'s description, which stays.
- Docs and copy: `docs/orchestration.md` (feature YAML, table, `graph list`
  output, workflow JSON), `docs/surfaces.md` (lines, sentences, stop counts),
  `docs/getting-started.md` ("pauses once more to verify on `main`"),
  `swift/README.md` workflow bar, `website/home.html` workflow cards,
  `.lf/qa/auto.md` (the `qaship` Graph's last step).

Stays: release `verify:` commands (`docs/config.md`, `docs/ops.md`,
`.lf/config.yaml`, `release-run.md`), `git … --verify`, the ordinary verb in
prose, release notes, `design/prototypes/` (dated mockups),
`scripts/evals/instruction_levels.*` (an unrelated action name).

## Exclusions

- The engine's support for an interactive step after a merging step.
- Editing `manabot`, `fantasia`, `etude`, `etude-sdk`.
- A post-merge check anywhere else.

## One pass

Graphs and skill first, then every reader in the list above, then:

```bash
cargo test -p loopflow-execution --lib task_graph::
cargo test -p loopflow-execution --lib graph::compiled::
cargo test -p loopflow-execution --lib skills::catalog::
cargo test -p loopflow --lib task_graph_launch_tests::
```

Swift view tests (`WorkflowLineTests`, `BacklogViewTests`,
`AppBootstrapResourceTests`) and the DTO fixture tests run in CI at gate.

## Demo and acceptance at gate

```console
$ lf graph list | grep -E '^(code|feature) '
code       Build and deliver. You review the change before it merges.
feature    Explore, build and deliver. You choose what to build and review the change before it merges.
$ lf graph list --json | jq -c '.[] | select(.name=="code") | .workflow | [.sentence, (.steps|map(.name))]'
["You take part at ship-review.",["plan","build","ship-review","deliver"]]
$ lf verify
skill or graph not found: verify
$ git grep -nw verify -- rust/loopflow-definitions/src/builtins/task
# only the verb in ci-fix.md, task_operate.md, ship-decomposed.md
```

Done when a fresh Task on `code` reads "You take part at ship-review.", its
run ends at `deliver` with the Task completed on merge, and Desktop's Start
menu shows `code` with 1 stop and `feature` with 2.

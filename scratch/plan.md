# LOO-1108: builtin Graphs end at deliver; the verify skill goes

## Outcome

`code` and `feature` end when `deliver` merges the PR. The Graph reaches `end`,
which requests completion automatically (`docs/work.md`; `quick` and
`deliver`-ending repo Graphs do this today). Nobody is asked to come back after
the merge.

`lf task run`'s `merges:` warning follows the same landing predicate that
runtime `auto off` enforcement uses (`task_graph::landing_name`), instead of
the `reaches_end` proxy that misreported both non-merging Graphs (`pursue`,
`deep-clean`, `optimize`, `incident`, `queue`, `task-design`) and custom Graphs
with a post-merge interactive step.

| | before | after |
|---|---|---|
| `code` line | plan build ☺ship-review deliver ☺verify | plan build ☺ship-review deliver |
| `code` sentence | You take part at ship-review and verify. | You take part at ship-review. |
| `code` in Start | 2 stops | 1 stop |
| `feature` sentence | You take part at explore, ship-review and verify. | You take part at explore and ship-review. |
| `feature` in Start | 3 stops | 2 stops |
| `lf verify` (in this repo and any other) | runs the skill | `skill or graph not found: verify` |
| `lf task run` from `ship-review` on `code` | no `merges:` line | prints `merges:` |
| `lf task run <ID> pursue` | prints `merges:` (false positive) | no `merges:` line |

## Stacking

This branch is stacked on LOO-1104 (PR #2047, open): `lf sync LOO-1104` ran
before any edit, as two steers asked ("Stack this Task on LOO-1104 by running
`lf sync LOO-1104` first, before editing code.yaml, feature.yaml, or
verify.md."). #2047 rewrites `verify.md` and adds `quick.yaml`; it does not
touch `code.yaml` or `feature.yaml`. If #2047 merges first, `lf sync main`
detaches. Never `git merge` its branch by hand.

## The Model

### 1. Builtin Graphs and skill removal

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

The outer `loop:` goes with `verify`. It existed for verify's "fix forward"
iterate, and it cannot stay: a loop's last step must be a skill
(`rust/loopflow-execution/src/graph/mod.rs`, "a loop's last step decides
whether to run it again"), and `deliver` ends in `cmd: land --wait-and-fix`.

Delete both `rust/loopflow-definitions/src/builtins/task/skill/verify.md` and
this repository's `.lf/skills/verify.md` (which has `extends: builtin` and
whose headless checking bullets already live in `.lf/skills/qa.md`). Without
deleting `.lf/skills/verify.md`, `lf verify` inside loopflow fails with
`no builtin skill is named "verify"` and `lf list skill` shows `verify` as
`unavailable`.

### 2. One landing-reachability predicate for `merges:`

`TaskRunSelection::reaches_end` (`rust/loopflow-execution/src/ops/task.rs`) has
one caller: computing `merges: bool` on `TaskRunStart::Run`. Conflating "no
interactive step ahead" with "merges the PR" breaks in both directions:
non-merging Graphs with no stops (`pursue`, `deep-clean`, `incident`,
`optimize`, `queue`, `task-design`, and `auto on` on any non-merging Graph or
`TaskRunStart::Live`) falsely warn `merges: this run may merge the PR`, while
custom Graphs with an interactive step after `deliver` silently merge with no
warning.

Replace `TaskRunSelection::reaches_end` with one method on `CapturedGraph` in
`rust/loopflow-execution/src/task_graph.rs`, right beside `landing_name` and
`next_interactive`:

```rust
/// Whether a run from `place` can reach a merging step (`landing_name`) before
/// stopping. When `through` is false, each path stops at an interactive step;
/// when `through` is true, scanning continues past interactive steps to `end`.
pub fn reaches_landing(
    &self,
    place: &Place,
    through: bool,
    merges: fn(&[String]) -> bool,
) -> bool
```

Semantics:
- Walk `&body[from..]` at the current level and ascend `place.path` when the
  body can exit without stopping:
  - `landing_name(step, merges).is_some()` → returns `true`.
  - `!through` and `step` is interactive → this path stops (`(false, true)`).
  - `ConcreteStep::Xor(branch)`: scan every path from `0`. If any path can
    reach a landing step, return `true`. Stop past the XOR only when `paths` is
    non-empty and every path stops interactively.
  - `ConcreteStep::Skill(skill)` with `returns: Some(target)` (loop back-edge):
    when not stopping at this step and `target < from`, also scan
    `&body[target..from]` once so a run starting inside a loop (such as
    `loop: [deliver, interactive: check]` with `auto on` at `check`) sees the
    merging step it can iterate back to.
- In `try_start_task_run` (`rust/loopflow-execution/src/ops/task.rs`):
  - `TaskRunStart::Live` gains `merges: bool`, computed from
    `row.graph.place_of(cursor).is_some_and(|p| row.graph.reaches_landing(&p, true, merges))`;
    `rust/loopflow/src/lf/commands/graph.rs` forwards it instead of hardcoding `true`.
  - For `TaskRunStart::Run`, read `auto` first (`on = AutoState::On`, `off = AutoState::Off`,
    `through = mode.runs_through() || on`), and set
    `merges = !off && graph.reaches_landing(&start.place, through, merges)`.
  - Update doc comments on `TaskRunStart::Run.merges` (`ops/task.rs`) and
    `TaskRun::Running.merges` (`engine/task.rs`) to match.
- Wire shape (`TaskRun::Running`) is unchanged.

## Findings

- **Tasks already on `code`/`feature` keep `verify`.** A Task stores its Graph
  compiled, skill text included (`CapturedGraph` holds `ConcreteSkill.skill`;
  `take_up_task_graph` in `rust/loopflow-execution/src/ops/task.rs`).
  `.lf/qa/auto.md` records the same for the earlier `human-review` rename: "A
  Task left waiting there ships after the upgrade (the captured Graph carries
  its skill)". No migration. This Task's own run is one of them.
- **Completion on merge works for leaves and parents.** Reaching `end` calls
  `arrive_task_graph(..., END)` → `request_completion_in` →
  `settle_pending_completion`, marking merged leaf Tasks `done` and leaving
  parents with open children open as `Merged · N children open`.
- **Loopflow's own `.lf/skills/verify.md` extends the builtin.** Deleting it in
  this PR keeps `lf list skill` and `lf verify` clean in this repository.
- **Other repositories name the builtin.** On this Machine:
  `manabot` and `fantasia` have `- interactive: verify` in
  `.lf/graphs/{coverage,engine-performance,experiment-performance}.yaml` with
  no skill of their own; `etude` and `etude-sdk` have `.lf/skills/verify.md`
  with `extends: builtin`. After a release with this change, those Graphs fail
  a new take-up with "skill not found: verify" and the `extends: builtin`
  skill has nothing to extend. The repair is a hand edit there (drop the step,
  or write the skill out in full). No loopflow code migrates them.

## Choices made for review

1. **Nothing replaces `verify`'s post-merge check** (reading post-merge CI for
   the merge SHA, checking claims against `main`, fix forward). Passed over:
   folding a post-merge check into `deliver` after `land` (adds a headless
   agent step behind every merge), and keeping `verify` as an unreferenced
   builtin (dead prose). Recovery after a bad merge uses `lf task reopen`, a
   new Task, or `lf incident`. Repositories that want a post-merge stop write
   `- interactive: <their own skill>` after `deliver`; the engine still
   supports an interactive step after a merging one.
2. **`lf task run`'s `merges:` line reports reachable landing steps** via
   `CapturedGraph::reaches_landing` instead of `TaskRunSelection::reaches_end`.
   Passed over: leaving `reaches_end` as-is (keeps false positives on
   `pursue`/`deep-clean`/`optimize`/`incident`/`queue`/`task-design` and false
   negatives on custom post-merge-stop Graphs).
3. **Desktop's `StartFlow.merges` wire consolidation stays outside LOO-1108**
   (`swift/LoopflowMac/Views/BacklogView.swift:128-130` prefix-checks `land` /
   `pr land`, missing `pr arm`, `pr-land`, and `xor` branches). Adding a
   required field to `WorkflowOutline` changes the DTO wire schema; keep it as
   a small follow-up on top of `reaches_landing`.

## What changes

- Delete `rust/loopflow-definitions/src/builtins/task/skill/verify.md` and
  `.lf/skills/verify.md`.
- Edit `rust/loopflow-definitions/src/builtins/task/graph/code.yaml` and
  `feature.yaml` (remove `- interactive: verify`, outer `loop:`, update
  descriptions).
- `rust/loopflow-definitions/src/builtins.rs`: add `"verify"` to
  `retired_skills_are_not_registered`.
- `rust/loopflow-execution/src/task_graph.rs`: add
  `CapturedGraph::reaches_landing` with unit tests (`code`, `feature`, `quick`,
  `pursue`, `research`, custom post-merge stop, `xor`, loop back-edge); update
  builtin tests (interactive names, "after the last interactive step" moves to
  `ship-review`, workflow line, `WorkflowOutline.sentence` doc comment).
- `rust/loopflow-execution/src/ops/task.rs`, `engine/task.rs`,
  `rust/loopflow/src/lf/commands/graph.rs`: remove `TaskRunSelection::reaches_end`,
  compute `merges` via `reaches_landing` for both `TaskRunStart::Run` and
  `TaskRunStart::Live`, update doc comments.
- Tests that assert builtin shape or borrow `verify` as a stand-in:
  - `rust/loopflow-execution/src/graph/compiled.rs` (steps and sentences for
    `code` and `feature`).
  - `rust/loopflow/tests/work/task_graph_launch_tests.rs`:
    `verified.yaml`, `verifying.yaml` borrow another builtin skill (behavior
    unchanged); update `auto_on_a_running_task_reports_its_run_and_starts_nothing`
    (whose `feature` fixture had no landing step and relied on hardcoded
    `merges: true`) to assert `merges:` on a landing Graph and no `merges:` on
    a non-landing Graph, plus a check that a custom post-merge-stop Graph
    prints `merges:` when crossing its landing step.
  - `rust/loopflow-execution/src/skills/catalog.rs` (#2047's `.lf/skills/verify.md`
    expecting `Replaces` uses a remaining builtin name).
  - `rust/loopflow-execution/src/ops/task_execution.rs` (step label).
- Swift: `swift/Loopflow/Models/Graph.swift` doc comment;
  `WorkflowLineTests.swift` (four steps, tone arrays shrink),
  `BacklogViewTests.swift` (`code` becomes 1 stop, `"1 stop"`),
  `AppBootstrapResourceTests.swift` (sample word).
- Fixtures: `tests/fixtures/dto/graph_catalog.json` and
  `tests/fixtures/dto/server_schema.json` descriptions. `skill_entries.json`
  has no `verify` entry; `debug`'s verb stays.
- Docs and copy (updated in `write-docs`):
  - `docs/getting-started.md` (`code` description: ends by delivering and merging)
  - `docs/orchestration.md` ("two high-leverage moments", `feature` YAML,
    `ship-review` handoff to `deliver`, Built-in Graphs table, `lf graph list`
    output, workflow JSON, `Auto` `merges:` wording)
  - `docs/surfaces.md` (New Task, unstarted Task, Start menu descriptions and stop counts)
  - `swift/README.md` (workflow bar)
  - `website/home.html` (workflow cards: removed `<b>verify</b>`; kept
    `<span>deliver →</span>` before `<i>end</i>` to match `research`'s
    `b::after` arrow and pre-#2012 `<span>ship →</span><i>end</i>`)
  - Remaining for implementation: `.lf/qa/auto.md` (`qaship` Graph's last step).

Stays: release `verify:` commands (`docs/config.md`, `docs/ops.md`,
`.lf/config.yaml`, `release-run.md`), `git … --verify`, the ordinary verb in
prose, release notes, `design/prototypes/`, `scripts/evals/instruction_levels.*`.

## Exclusions

- The engine's support for an interactive step after a merging step.
- Moving Swift's `StartFlow.merges` heuristic onto a `WorkflowOutline` wire
  field (separate follow-up).
- Editing `manabot`, `fantasia`, `etude`, `etude-sdk`.
- A post-merge check anywhere else.

## One pass

1. Delete both `verify.md` files, update `code.yaml`, `feature.yaml`, and
   `builtins.rs`.
2. Add `CapturedGraph::reaches_landing` and switch `ops/task.rs`,
   `engine/task.rs`, and `commands/graph.rs` off `reaches_end`.
3. Update Rust tests, Swift comments/tests, DTO fixtures, and docs/copy.
4. Run focused checks (with `LF_*` unset and `< /dev/null` where needed):

```bash
cargo test -p loopflow-definitions --lib builtins::
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

Done when:
- a fresh Task on `code` reads `"You take part at ship-review."` and completes
  when `deliver` merges its PR;
- `lf verify` in this repository prints `skill or graph not found: verify` and
  `lf list skill` lists no `verify`;
- `lf task run` prints `merges:` when crossing `deliver` on `code` (or a custom
  post-merge-stop Graph) and omits `merges:` on `pursue`;
- Desktop's Start menu shows `code` with 1 stop and `feature` with 2.

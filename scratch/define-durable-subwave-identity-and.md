# Subwaves (Infrastructure · LOO-354)

Jack Heart reopened this work on 2026-09-30 after abandoning LOO-329 and
closing #1318. The accepted model below comes from the 2026-09-28 review;
`ded3dd02a:scratch/subwave-identity-review.md` retains the review record.
Restored from `backup/define-durable-subwave-identity-and-20260930`.
Companions: Product LOO-330 and Intelligence LOO-331.

This branch starts at current LOO-298 commit `ab901f1f16e19e663791e635bbc85366b8a2b59e`.
Only LOO-329's read-slice changes were ported; none of its inherited migration
copies or older LOO-298 commits were brought over. Keep synchronization by merge,
never rebase. Jack requested no design review; interactive acceptance is at demo.
Inherited LOO-298 scratch notes were removed under the supervising session's
2026-09-30 steer; their original contents remain in the base commit's history.

## Remaining work

1. Local implementation and the disposable-Home proof are complete. The
   supervising Flow owns integration, broader gate, publication
   and Jack's interactive demo. Stay on the code Flow until LOO-298 lands;
   synchronize by merge, never rebase. No push or landing ran in this pass.
2. After landing, use installed `lf` to create Release's Linear Initiative and
   plan, move release-focused Tasks/KRs and sync the live schedule. The authored
   schedule now belongs to release; no provider or launchd cutover occurred here.
3. Configured provider/Desktop acceptance remains the demo's work. The disposable
   store proofs below establish local Rust behavior, not installed acceptance.

## Implementation and proof · 2026-09-30

Sync proof: `cargo test -p loopflow --lib -- large_task_launch_stays_within_context_budget_and_preserves_sources reference_rendering_preserves_live_requests_and_original_components attributed_context_keeps_escaped_reference_sources skill_launch_seed_activates_only_the_selected_skill_from_references implement_launch_treats_kickoff_plan_and_intent_as_references skill_exec_seed_carries_wave_files_before_the_message` — five passed; the ordering assertion was corrected to match file tags and passed on its focused rerun; `cargo test -p loopflow --test golden_prompt` passed. Logs: `.lf/tmp/subwaves/sync-verification.log`, `sync-golden.log`; disposable Home, inherited authority cleared, 41.9 GiB resource preflight. Broader verification and push remain with the supervising Flow.

The Release follow-through uses the authored Infrastructure and Release Markdown
in a disposable checkout/Home through the compiled `lf-prompt` executable. Both
selected-Wave prompts passed: each selected file appears once, ancestors precede
Release, the parent excludes Release, ordinary context excludes auth/Product and
child directories, and nested scratch remains included. Authored configuration
checks confirm Infrastructure's 09:00 telemetry schedule, Release's 10:00
release-run schedule and its mechanical `release run patch` Flow. No scheduled
release was invoked. The existing disposable-store Task-binding, rename,
reparenting and fresh-Home regression remains the Task/identity proof; this new
check exercises real repository content without replacing that regression.

Release's GOAL now states its planning responsibility; its memory introduction
records the accepted nested-Wave scope and pending live cutover. Review removed
stale exploratory wording, deleted the two inherited parent notes, scoped
`scratch/questions.md` to LOO-354 and replaced the dangling parent-memory pointer
with its preserved Git location. No Rust implementation changed in this pass.

Proof command: `uv run python .lf/tmp/subwaves/release-proof.py`.
Outputs: `.lf/tmp/subwaves/release-proof.log`, `release-proof-build.log`,
`release-proof-inputs.json` and the two `*-authored-prompt.md` files beside them.
The script removes its temporary checkout/Home and strips inherited LF_/LOOPFLOW_
authority. Resource preflight passed at 37.7 GiB free above the 32 GiB reserve;
the busy uv cache was retained. This is local authored-content proof, not Linear,
launchd, configured provider or Desktop acceptance. Formatting, architecture
coverage and diff checks passed. No broader gate ran.

The final reader commands passed (103 tests across seven commands), resolving the
previous resource stop. [Read-slice evidence](subwave-read-slice.md) retains the
failed preflights and the successful resume.

Wave rows now hold a one-segment name and directory parent. The `wave_addresses`
SQL view derives paths; Wave snapshots, prompt binding, planning, metrics,
Session/history selectors, CI labels and relocation consume that address. The
forward `wave_directory_parents` draft preserves Wave/Project references, derives
parents from the old full-path names and supplies absent ancestors. Old promotion
links do not override directory parentage; `promoted_at` remains historical evidence.
Released migrations and LOO-298's three drafts are unchanged.

Discovery carries the Wave UUID in existing GOAL.md frontmatter, adopting the
registered UUID when no `id:` is present. This resolves the previously unspecified
move evidence without a sidecar or rename ledger. Parent rename updates only the
parent row; reparenting changes the child's parent, preserving its ID and Task
links. Same leaf names under different parents are independent. Conflicting
copied IDs, cycles and cross-repository directory parents are rejected.

The authored release GOAL now owns the 10:00 release-run schedule. Infrastructure
retains telemetry and its objective. Cron installation creates the complete nested
log directory before launchd could open it.

Concrete review findings fixed: redundant reader deduplication; SQL selectors
still treating a leaf as a full address; relocation rewriting descendant rows
on a parent rename; missing destination-parent discovery; nested cron log creation;
and schema verification omitting SQL views. The obsolete relocation fixture for
an unparented full-path Wave was removed because registration now requires segment
names and records directory ancestry. The minimal identity-reader proof retains
its lack of execution tables and uses the current address view.

Focused proofs use disposable stores and synthetic provider/launchctl effects:
parent rename leaves the child row unchanged; directory reparenting and a fresh
Home preserve the authored ID; a bound release Task gathers ancestor then child
memory exactly once, excluding auth/Product siblings; migration retains Project
links; nested Task-history selectors and relocation preserve identity; scheduled
launch carries the nested Wave through the real CLI argument parser.
Logs: `.lf/tmp/subwaves/final-parent-verification.log` and
`.lf/tmp/subwaves/schema-review-verification.log` (final review checks).

The final parent run passed 48 selected tests: populated migration (1), Wave
module (34), binding/history (7), stored ancestry (1), nested scheduled run (1),
and repository relocation (4). The final schema-review run passed the migration
and minimal-reader checks after adding view verification. All-target Clippy,
formatting, diff checks, migration-history validation (56 shipped migrations
unchanged) and architecture checks passed. Website doc copies were regenerated.
These are focused implementation checks; no affected-suite or full gate ran.

Compression removes the old relocation fallback for children with unrelated
paths. Directory parentage now supplies each child's destination directly;
the move list itself drives the parent-first traversal. Discovery fetches only
the final Wave after reconciliation and skips redundant updates of newly created
rows. Parent validation and relocation SQL are expanded for readability.
The existing relocation proof now includes a grandchild through rename, repository
move and recovery, retaining its identity and parent links.

Compression verification passed 45 tests (34 Wave, seven binding/history and
four repository relocation), all-target Clippy, formatting and diff checks.
Log: `.lf/tmp/subwaves/compress-verification.log`. The run used a disposable Home
with inherited LF_/LOOPFLOW_ authority removed; resource preflight reported
40.9 GiB free above the 32 GiB reserve. Prior prompt and migration evidence remains
recorded above; this pass did not rerun those suites or establish installed/demo
acceptance.

A second compression review found that checkout-local document gathering had
removed `load_goal`'s last runtime caller. Wave context now renders its operating
instruction and available Flow list directly. The unused `Goal` wrapper, goal
loader/fallback search, goal errors, builtin goal registration and six unreachable
goal assets are removed together. S1–S5 skills and Flows remain registered; this
removes the separate goal catalog, not their operating instructions. The removed
Rust goal APIs have no repository callers; external source consumers were not
inventoried. Goals and memory continue through `gather_context`.

The two synthetic goal-seed prompt tests are folded into the existing checkout
binding proof, which now checks that goal, memory and LOOPFLOW instructions each
appear once. The direct Wave binding proof also checks local and builtin Flow
names alongside metrics. This keeps proof at the user path instead of retaining
an unused loader for its tests. Parent identity and migration code are unchanged.

This follow-up passed 150 focused tests (42 Flow, 72 prompt, 13 builtin catalog,
seven Work binding/history and 16 context), all-target Clippy, formatting,
architecture coverage and diff checks. Evidence is in
`.lf/tmp/subwaves/compress-followup-verification.log`; resource preflight passed
at 45.9 GiB free in `compress-followup-resource.log`. Checks used a disposable
Home, cleared inherited LF_/LOOPFLOW_ authority and pinned the checkout CLI.
No full gate, provider/Desktop demo, live planning or schedule operation ran.

## The dream

You say `infrastructure/release` and every part of Loopflow means the same
place. An agent working on release knows what Infrastructure knows and what
release has learned, and nothing about auth or growth. A big memory splits
into smaller ones as naturally as a big directory does. It works in any
checkout, including a fresh clone.

Before this slice, `wave/infrastructure/release/MEMORY.md` existed
and reached no prompt.

## The model

A Wave has an id, a name, and a parent.

| | Infrastructure | Release |
|---|---|---|
| name | `infrastructure` | `release` |
| parent | none | Infrastructure |
| you type | `infrastructure` | `infrastructure/release` |
| files | `wave/infrastructure/` | `wave/infrastructure/release/` |

A subwave is a Wave. It has its own objective, plan, Tasks and schedule.

A parent's objective may include its subwaves' goals. Its KRs and Tasks
should not focus on a subwave's work: work that is mainly about release
belongs in release's plan. Mentioning release is fine. Jack Heart,
2026-09-28. This is guidance for whoever writes the plan, and nothing
enforces it.

A Wave's directory sits inside its parent's. Discovery records the parent
from the directory; moving the directory changes the parent. Jack Heart,
2026-09-28.

The id is what records point at, so renaming `infrastructure` to `infra`
changes one row and release is untouched. Release is then reached as
`infra/release`, because the path is worked out from names and stored nowhere.

A Task reaches its Wave through its Project, by id, as LOO-298 has it.
Jack Heart asked only that it be done right. A Wave stored on the Task is
added when a read needs it, with LOO-298's check.

## What a Run reads

Every top-level `.md` in its Wave's directory and in each parent's, root
first, from the Run's own checkout.

```
at infrastructure/release:

read     wave/infrastructure/*.md
read     wave/infrastructure/release/*.md
skipped  wave/infrastructure/auth/      sibling
skipped  wave/product/, …               other Waves
```

- The Wave comes from the Task the Run belongs to, or from `--wave`. How the
  Run was started does not matter: scheduled and ad hoc Runs follow the same
  rule. Jack Heart, 2026-09-28.
- The Wave file stays `GOAL.md`. Jack Heart considered `<NAME>.md` and chose
  `GOAL.md`, 2026-09-28.
- Memory is one of those files, not a special case.
- No children, no siblings, in ordinary Runs.
- `realign` at a parent reads its children's files. It is the one exception,
  and it is how a child's findings reach the parent's memory. Jack Heart,
  2026-09-28.
- There is no single Wave memory. It is whatever this checkout holds, and an
  edit is an ordinary change that lands with the PR.
- `scratch/` stays recursive.
- A short note says which memory is yours to curate. How to curate lives in
  `realign`, which reconciles plan, code and Wave memory together.

Approximate cost today: an Infrastructure Run carries 18,900 tokens from
`wave/`. A release Run would carry about 20,000.

## Trying it: release

Release becomes a real Wave. That is the prototype.

```markdown
---
crons:
- flow: release-run
  schedule: 0 0 10 * * *
---

## Objective

Deliver new and updated Loopflow to the public at regular intervals, with
accessible guidance, without bugs, through a smart and recoverable rollout.
```

The objective is Jack's sentence, lightly edited at his request.
Infrastructure's objective keeps "delivers verified releases". KRs and open
Tasks mainly about release move to release's plan at the split. The split
creates a Linear Initiative and moves a live schedule, so it runs from the
installed `lf` after the code lands.

## Build

1. **The read.** One gatherer that follows the path. It replaces the separate
   memory function and its registry walk, and needs no registry.
2. **The parent.** Finding `wave/infrastructure/release/GOAL.md` records
   release with Infrastructure as its parent. Names become one segment.
3. **Release.** Make it a Wave and use it.

Prove in a throwaway Home that a release Task's prompt carries
Infrastructure's files then release's, and renaming the parent directory
leaves release's row alone.

## Later

- Rename and reparent commands. `lf wave relocate` stays as LOO-298 leaves it.
- Conflicts between machines. For now two machines sync a Wave when nothing
  conflicts.
- The desktop app.
- Moving Run history on rename.

## Watch for

- **Parent discovery.** Directory reconciliation now populates parent IDs and
  follows authored UUIDs through moves. Duplicate IDs remain explicit conflicts.
- **Cron logs.** Nested addresses add a directory. Installation and scheduled
  spawning both create the full parent path; the scheduled fixture covers it.
- **Memory conflicts.** Two Tasks curating one 18,400-token file will meet at
  rebase. Subwaves make that rarer.
- **The written contract.** The architecture reference now distinguishes stored
  one-segment names from derived addresses.

## Not doing

- A second kind of thing beside a Wave for memory-only directories.
- A check that refuses when directory and parent field differ.
- Running a branch build against the installed Home.

## Where this came from

The kickoff draft (`0381a195f`) proposed dropping the parent field, a rename
ledger and memory-only scopes. Review first moved to the name as the identity
(`2f54a48f7`), then back to ids once Jack saw that renaming a parent would
rename every child. Source findings, measurements and the cron research behind
this page are in those commits and in `11ff4121c`.

## Read-slice implementation

The existing document pipeline now collects ancestor and selected Wave Markdown
from the executing checkout. Memory has no separate registry walk or launch
input. Native seeds share rendering and preserve memory's accounting category.
Review also removed duplicate goal-body injection and preserved the invoking
checkout for direct same-repository Wave selection. Explicit documentation or
changed-file selection of an already included Wave file renders it once.

The final reader regressions and golden passed after capacity recovered.
[Read-slice evidence](subwave-read-slice.md) retains both the earlier failure
and the successful resume.

# Subwaves (Infrastructure · LOO-329)

Shaped in review-design with Jack Heart on 2026-09-28. Stacked on
[LOO-298](https://linear.app/loopflow/issue/LOO-298)
([#1296](https://github.com/loopflowstudio/loopflow/pull/1296)) at
`ce97003cf`, which includes main through #1310.
What Jack said, in order: `ded3dd02a:scratch/subwave-identity-review.md`
(retained in Git; branch preparation removed the working copy).
What is still open: [subwave-questions.md](subwave-questions.md).
Companions: Product [LOO-330](https://linear.app/loopflow/issue/LOO-330),
Intelligence [LOO-331](https://linear.app/loopflow/issue/LOO-331).

Build step 1 is coded but integrated verification is blocked by inherited
LOO-298 compilation errors. Parent discovery and the release split remain to
build; the disposable-Home proof has not run. The inherited LOO-298
implementation is outside this branch's changes.

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

- **Parent discovery.** The kickoff audit found the parent field empty
  everywhere. Step 2 must populate it and reconcile moved directories.
- **Cron logs.** The log path uses the Wave name as written, so
  `infrastructure/release` adds a directory. `add_cron` creates only
  `.lf/logs`; `spawn_cron_target` creates the full parent path. Verify the
  scheduled launch path. Read from `ops/cron.rs`, not run.
- **Memory conflicts.** Two Tasks curating one 18,400-token file will meet at
  rebase. Subwaves make that rarer.
- **The written contract.** LOO-298's architecture reference describes Wave
  names as whole paths. Step 2 changes those lines first.

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

## This slice

2026-09-28: the checkout-local ancestor read uses the existing
prompt document pipeline for every Wave Markdown file, including memory.
The separate Work-layer memory registry walk and preassembled memory input
are removed. Native skill launch context and context attribution use that same
collection. Parent identity/discovery and the installed release split remain
later build steps; this slice cannot establish their disposable-Home proof.

Proof and open assumptions: [read slice ledger](subwave-read-slice.md).

# Subwaves (Infrastructure · LOO-329)

Shaped in review-design with Jack Heart on 2026-09-28. Stacked on
[LOO-298](https://linear.app/loopflow/issue/LOO-298)
([#1296](https://github.com/loopflowstudio/loopflow/pull/1296)).
What Jack said, in order: [subwave-identity-review.md](subwave-identity-review.md).
What is still open: [subwave-questions.md](subwave-questions.md).
Companions: Product [LOO-330](https://linear.app/loopflow/issue/LOO-330),
Intelligence [LOO-331](https://linear.app/loopflow/issue/LOO-331).

## The dream

You say `infrastructure/release` and every part of Loopflow means the same
place. An agent working on release knows what Infrastructure knows and what
release has learned, and nothing about auth or growth. A big memory splits
into smaller ones as naturally as a big directory does. It works in any
checkout, including a fresh clone.

Today none of that is true. `wave/infrastructure/release/MEMORY.md` exists on
main and reaches no prompt.

## The model

A Wave has an id, a name, and a parent.

| | Infrastructure | Release |
|---|---|---|
| name | `infrastructure` | `release` |
| parent | none | Infrastructure |
| you type | `infrastructure` | `infrastructure/release` |
| files | `wave/infrastructure/` | `wave/infrastructure/release/` |

A subwave is a Wave. It has its own objective, plan, Tasks and schedule.

A parent's objective may include its subwaves' goals. Its KRs and Tasks do
not: release's KRs and Tasks belong to release's plan, and Infrastructure's
plan does not mention them. Jack Heart, 2026-09-28.

A Wave's directory sits inside its parent's. Release lives in
`wave/infrastructure/` only because its parent is Infrastructure. Jack Heart,
2026-09-28. The directory is what you edit; the parent is recorded from it,
and moving the directory is how a Wave changes parent.

The id is what records point at, so renaming `infrastructure` to `infra`
changes one row and release is untouched. Release is then reached as
`infra/release`, because the path is worked out from names and stored nowhere.

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
- Memory is one of those files, not a special case.
- No children, no siblings.
- There is no single Wave memory. It is whatever this checkout holds, and an
  edit is an ordinary change that lands with the PR.
- `scratch/` stays recursive.
- A short note says which memory is yours to curate. How to curate lives in
  `update-wave`.

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
Infrastructure's objective keeps "delivers verified releases". Release KRs
and open release Tasks in Infrastructure's plan move to release's at the
split. The split creates a Linear Initiative and
moves a live schedule, so it runs from the installed `lf` after the code
lands.

## Build

1. **The read.** One gatherer that follows the path. It replaces the separate
   memory function and its registry walk, and needs no registry.
2. **The parent.** Finding `wave/infrastructure/release/GOAL.md` records
   release with Infrastructure as its parent. Names become one segment.
3. **Release.** Make it a Wave and use it.

Shown working in a throwaway Home: a release Task's prompt carries
Infrastructure's files then release's, and renaming the parent directory
leaves release's row alone.

## Later

- Rename and reparent commands. `lf wave relocate` stays as LOO-298 leaves it.
- Conflicts between machines. For now two machines sync a Wave when nothing
  conflicts.
- The desktop app.
- Moving Run history on rename.

## Watch for

- **Two hierarchies.** Directories and the parent field both say who the
  parent is, and they must agree. The kickoff audit found the field empty
  everywhere, which is why nesting did nothing. Step 2 makes discovery write
  it, so a moved directory is re-recorded and nothing has to refuse.
- **Release files are missing on this base.** LOO-298 branched before main
  gained release memory and the `release-run` Flow file
  ([#1311](https://github.com/loopflowstudio/loopflow/pull/1311)).
- **Cron logs.** The log path uses the Wave name as written, so
  `infrastructure/release` points into a directory nothing creates. Read from
  `ops/cron.rs`, not run.
- **Memory conflicts.** Two Tasks curating one 18,400-token file will meet at
  rebase. Subwaves make that rarer.
- **The written contract.** LOO-298's architecture reference describes Wave
  names as whole paths. Step 2 changes those lines first.

## Not doing

- A second kind of thing beside a Wave for memory-only directories.
- Anything in a prompt from a sibling or child.
- A check that refuses when directory and parent field differ.
- Running a branch build against the installed Home.

## Where this came from

The kickoff draft (`0381a195f`) proposed dropping the parent field, a rename
ledger and memory-only scopes. Review first moved to the name as the identity
(`2f54a48f7`), then back to ids once Jack saw that renaming a parent would
rename every child. Source findings, measurements and the cron research behind
this page are in those commits and in `11ff4121c`.

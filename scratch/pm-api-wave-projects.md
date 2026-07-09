# lf op pm: wave/project/task API pass

## Problem

The new planning model is:

- wave = durable operating context
- project = measured bet inside exactly one wave, stored at `wave/<wave>/projects/<project>.md`
- task = concrete work in Linear

`lf op pm` still exposes the old model:

- a wave has one Linear "project"
- `show` prints all Linear issues for that wave
- `update` creates/edits/closes issues
- there is no operation for local project docs, Linear project renames, task
  migration, or checking that Linear still matches the local wave/project shape

That mismatch showed up immediately after renaming the roster. Local waves became
`product`, `intelligence`, and `infrastructure`, but Linear still contained the
old task sets and project names. The datamodel Linear project became stranded
because no local wave points at it.

## Terms

Keep the product language consistent:

- **Wave**: local `wave/<wave>/`.
- **Project**: local `wave/<wave>/projects/<project>.md`.
- **Task**: provider issue, currently Linear issue.
- **PM space**: the provider container currently called a Linear project.

Avoid using "project" for both local measured bets and Linear containers in user
commands and docs.

## Target commands

```bash
lf op pm status
lf op pm doctor
lf op pm sync --plan
lf op pm sync

lf op pm show --wave product
lf op pm show --wave product --project wave-chat
lf op pm update --wave product --project wave-chat --title "..."
lf op pm update --wave product --project wave-chat --id <task-id> --status done

lf op pm space rename --wave product --title "Product"
lf op pm task move --id <task-id> --wave product --project loopflow-api
lf op pm task import --from-wave datamodel --to-wave product --project loopflow-api
```

`show` should group tasks by local project when the provider data has enough
metadata. Until then, it can show "unassigned" tasks and make the mismatch
visible.

## Provider mapping

Linear remains the task store. A local wave can still point at one Linear
container through `pm.linear_project`, but Linear tasks need a project
association matching the local project slug.

Preferred representation: Linear labels named `project:<slug>`.

Why labels:

- They are visible in Linear.
- They do not require one Linear project per local project.
- They allow a task to stay inside the wave's Linear container while attaching
  to a local measured bet.
- Existing tasks can be migrated incrementally.

`lf op pm update --project wave-chat` creates or ensures the `project:wave-chat`
label and attaches it to the issue.

## Sync behavior

`lf op pm sync --plan` should read:

- local waves and `pm.*_project` frontmatter
- local project docs
- Linear container names
- Linear tasks and `project:<slug>` labels

Then report:

- linked Linear containers whose title differs from the local wave title
- local waves with no PM space
- PM spaces no local wave points to
- tasks with no `project:<slug>` label
- tasks whose `project:<slug>` label names no local project
- local projects with no open tasks

`lf op pm sync` should apply only low-risk changes:

- rename PM spaces to the wave title
- create missing `project:<slug>` labels
- label tasks when an unambiguous migration rule exists

Ambiguous task moves stay in the plan output. Do not silently guess.

## Immediate migration for this PR

After the API exists, run a migration plan for:

- old `mac` Linear container -> `product`
- old `quality` Linear container -> `intelligence`
- old `systems` Linear container -> `infrastructure`
- old `datamodel` Linear container -> import relevant Flowloop tasks into
  `product / loopflow-api`; import architecture cleanup into
  `infrastructure / technical-architecture`

The current `lf op pm` can show and edit tasks, but it cannot rename Linear
containers or move/import tasks between PM spaces. Do not try to complete this
migration manually through ad hoc commands.

## Implementation notes

- `rust/loopflow/src/lfd/pm/linear.rs` already has project create/list and issue
  create/update/complete/comment primitives.
- Add provider methods for Linear project rename, issue project move if needed,
  label create/list/attach, and issue label reads.
- Rename user-facing docs from "roadmap" to "tasks" or "PM tasks"; keep
  "project" reserved for local measured bets.
- Fix the GraphQL ID mismatch while touching this layer: several Linear
  mutations use `String!` where Linear expects `ID!`.

## Done when

- `lf op pm status` shows the three current waves with the right PM space names.
- `lf op pm show --wave product --project wave-chat` filters by local project.
- `lf op pm update --wave product --project wave-chat --title "..."` creates a
  labeled Linear task.
- `lf op pm sync --plan` surfaces stranded PM spaces and unlabeled tasks.
- The docs no longer call the Linear container a project in the same breath as
  local measured-bet projects.

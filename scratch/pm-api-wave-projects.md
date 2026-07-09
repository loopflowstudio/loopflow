# lf op pm: wave/project/task API pass

## Problem

The planning model is now three nouns:

- **wave** = durable operating context
- **project** = measured bet inside exactly one wave, stored at
  `wave/<wave>/projects/<project>.md`
- **task** = concrete work in Linear

`lf op pm` still behaves like the old model:

- a wave has one Linear project
- `show` prints every Linear issue in that wave's Linear project
- `update` creates/edits/closes Linear issues
- tasks cannot be attached to a local Loopflow project
- Linear projects cannot be renamed after a wave rename
- Linear cannot show that it is out of sync with the local wave/project tree

That mismatch showed up in this branch. Local waves became `product`,
`intelligence`, and `infrastructure`, but Linear still had old project names
and old task groupings. The old `datamodel` Linear project became stranded
because no local wave points at it.

## Language

Keep the user-facing model to three nouns:

- **Wave**: local `wave/<wave>/`.
- **Project**: local `wave/<wave>/projects/<project>.md`.
- **Task**: Linear issue.

Each wave has one Linear project that holds its tasks. Do not introduce a fourth
word like "space" for this. Say "Linear project" when talking about Linear, and
"Loopflow project" or just "project" when talking about the measured bets under
a wave.

## Target CLI

Minimum viable API:

```bash
lf op pm status
lf op pm doctor
lf op pm sync --plan
lf op pm sync

lf op pm init --wave product
lf op pm rename --wave product --title "Product"

lf op pm show --wave product
lf op pm show --wave product --project wave-chat

lf op pm task create --wave product --project wave-chat --title "..."
lf op pm task update --id <task-id> --title "..." --notes "..."
lf op pm task done --id <task-id> --pr <url>
lf op pm task move --id <task-id> --wave product --project loopflow-api
```

Compatibility:

```bash
lf op pm update --wave product --project wave-chat --title "..."
lf op pm update --wave product --project wave-chat --id <task-id> --status done
```

`update` can remain as an alias for the new `task` subcommands, but the
documented path should become `task create/update/done/move` because it matches
the model.

## Command behavior

### `lf op pm status`

Show:

- each local wave
- linked Linear project id and title
- open/total task count
- unassigned task count
- task count per local Loopflow project
- warnings for Linear projects no local wave points to

Example:

```text
product: Linear project Product — 12 open / 18 total, 2 unassigned
  loopflow-api            4 open
  wave-chat               3 open
  auditability            1 open
  distributed-computing   1 open
  product-performance     1 open
  mac-surface-ux          2 open
  ios-surface-ux          0 open

stranded: Linear project Datamodel — 8 open / 12 total, no local wave points here
```

### `lf op pm show`

Without `--project`, group tasks by local project, then show unassigned:

```bash
lf op pm show --wave product
```

With `--project`, filter to that measured bet:

```bash
lf op pm show --wave product --project wave-chat
```

### `lf op pm task create`

Create a Linear issue in the wave's Linear project and attach it to the local
Loopflow project.

```bash
lf op pm task create --wave product --project wave-chat --title "Retain steward thread after restart"
```

Rules:

- `--project` must name an existing `wave/<wave>/projects/<project>.md`.
- The Linear task gets a visible association with that project.
- If `--project` is omitted, create the task as unassigned and warn.

### `lf op pm task move`

Move or relabel an existing task to a target wave/project:

```bash
lf op pm task move --id <task-id> --wave product --project loopflow-api
```

Rules:

- If the task is already in the target wave's Linear project, update only the
  local project association.
- If it belongs to another wave's Linear project, move it when Linear supports
  that cleanly; otherwise clone/link and clearly mark the source task as moved.
- Require `--project` to point at an existing local project.

### `lf op pm rename`

Rename the Linear project backing a wave:

```bash
lf op pm rename --wave product --title "Product"
```

This makes Linear match the wave name after local wave renames.

### `lf op pm sync`

`sync --plan` reads:

- local waves and `pm.linear_project` frontmatter
- local project docs
- Linear project names
- Linear tasks and their local project associations

Then reports:

- linked Linear projects whose title differs from the local wave title
- local waves with no Linear project
- Linear projects no local wave points to
- tasks with no local project association
- tasks whose local project association names no local project
- local projects with no open tasks

`sync` applies only low-risk changes:

- rename Linear projects to match wave titles
- create missing local project labels
- label/reassociate tasks when an unambiguous rule exists

Ambiguous task moves stay in the plan output. Do not silently guess.

## Linear mapping

Preferred representation: Linear labels named `project:<slug>`.

Why labels:

- They are visible in Linear.
- They do not require one Linear project per local Loopflow project.
- They allow every task to stay inside the wave's Linear project while
  attaching to a local measured bet.
- Existing tasks can be migrated incrementally.

`lf op pm task create --project wave-chat` creates or ensures the
`project:wave-chat` label and attaches it to the issue.

## Immediate migration demo

The new API should demonstrate this branch's exact workflow:

```bash
lf op pm sync --plan
```

Expected plan:

- rename old Mac Linear project to Product
- rename old Quality Linear project to Intelligence
- rename old Systems Linear project to Infrastructure
- report old Datamodel Linear project as stranded
- report old Product/Mac tasks as unassigned to local projects
- report old Intelligence/Quality tasks as unassigned to local projects
- report old Infrastructure/Systems tasks as unassigned to local projects

Then:

```bash
lf op pm rename --wave product --title "Product"
lf op pm rename --wave intelligence --title "Intelligence"
lf op pm rename --wave infrastructure --title "Infrastructure"

lf op pm task move --id <flowloop-task> --wave product --project loopflow-api
lf op pm task move --id <one-system-task> --wave infrastructure --project technical-architecture
lf op pm task move --id <memory-task> --wave product --project wave-chat

lf op pm show --wave product --project wave-chat
lf op pm show --wave product --project loopflow-api
lf op pm show --wave infrastructure --project technical-architecture
```

## Implementation notes

- `rust/loopflow/src/ops/pm.rs` owns the command-level API and currently calls
  Linear methods for project create/list and issue create/update/complete.
- `rust/loopflow/src/lfd/pm/linear.rs` already has Linear project create/list
  and issue create/update/complete/comment primitives.
- Add Linear methods for:
  - project rename
  - issue label list/create/attach/remove
  - issue project move if Linear supports it
  - task lookup by id
  - Linear project listing for stranded detection
- Rename user-facing docs from "roadmap" to "tasks" or "Linear tasks"; keep
  "project" reserved for local measured bets except when explicitly saying
  "Linear project."
- Fix the Linear GraphQL ID mismatch while touching this layer: several
  mutations use `String!` where Linear expects `ID!`.

## Done when

### API demos

- `lf op pm status` shows Product, Intelligence, and Infrastructure with Linear
  project names and task counts grouped by local project.
- `lf op pm show --wave product` groups tasks by local project and shows
  unassigned tasks separately.
- `lf op pm show --wave product --project wave-chat` shows only Wave Chat
  tasks.
- `lf op pm task create --wave product --project wave-chat --title "Retain steward thread after restart"` creates a Linear issue visibly associated with `project:wave-chat`.
- `lf op pm task move --id <task-id> --wave product --project loopflow-api`
  moves or relabels an existing task and `show --project loopflow-api` displays
  it.
- `lf op pm rename --wave product --title "Product"` renames the Linear project
  backing the Product wave.
- `lf op pm sync --plan` reports stranded Linear projects, unassigned tasks,
  invalid project associations, and local projects with no tasks.

### Workflow demos from this branch

- Starting from the pre-migration Linear state, `lf op pm sync --plan` identifies
  the old Datamodel Linear project as stranded.
- Flowloop work can be moved from the old Datamodel Linear project into
  `product / loopflow-api`.
- One-system cleanup work can be moved into
  `infrastructure / technical-architecture`.
- Memory work can be moved into `product / wave-chat`, and Intelligence has no
  standalone Memory project in either local files or task grouping.
- Product's task view can be filtered independently for Wave Chat, Loopflow API,
  Product Performance, Distributed Computing, Mac Surface UX, and iOS Surface
  UX.

### Documentation demos

- `docs/lfop.md` documents wave/project/task PM commands without exposing
  "space" or "provider container" as user-facing nouns.
- `docs/wave-authoring.md` says local projects live in
  `wave/<wave>/projects/*.md` and Linear holds tasks.
- User-facing docs avoid saying "project" ambiguously. Use "Linear project" for
  the Linear object and "project" for Loopflow measured bets.

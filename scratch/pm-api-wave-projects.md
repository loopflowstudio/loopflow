# lf op pm: wave/project/task API pass

## Problem

The planning model is now three nouns:

- **wave** = durable operating context
- **project** = measured bet inside exactly one wave, stored at
  `wave/<wave>/projects/<project>.md`
- **task** = concrete work in Linear

`lf op pm` still exposes the old model:

- a wave points at one Linear project
- `show` prints all Linear issues for that wave
- `update` creates/edits/closes issues
- there is no way to attach a task to a local project, migrate tasks between
  waves, rename the provider container after a wave rename, or detect that
  Linear no longer matches the local wave/project tree

That mismatch showed up in this branch. Local waves became `product`,
`intelligence`, and `infrastructure`, but Linear still contained old task sets
and old provider-container names. The old `datamodel` Linear project became
stranded because no local wave points at it.

## Language

Keep the CLI and docs to three nouns:

- **Wave**: local `wave/<wave>/`.
- **Project**: local `wave/<wave>/projects/<project>.md`.
- **Task**: provider issue, currently Linear issue.

Do not expose "space" as a user-facing noun. Internally, one wave is backed by
one provider container; in Linear that provider container is a Linear project.
The code may call this `ProviderContainer`, `PmContainer`, or `LinearProject`,
but the CLI should not ask users to reason about a fourth planning noun.

Docs can say:

> Each wave is backed by one provider container. In Linear, that container is a
> Linear project. Loopflow reserves "project" for measured bets under a wave.

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
- linked provider container id and display name
- open/total task count
- unassigned task count
- task count per local project
- warnings for provider containers no local wave points to

Example:

```text
product: Linear Product — 12 open / 18 total, 2 unassigned
  loopflow-api            4 open
  wave-chat               3 open
  auditability            1 open
  distributed-computing   1 open
  product-performance     1 open
  mac-surface-ux          2 open
  ios-surface-ux          0 open

stranded: Linear Datamodel — 8 open / 12 total, no local wave points here
```

### `lf op pm show`

Without `--project`, group tasks by local project label, then show unassigned:

```bash
lf op pm show --wave product
```

With `--project`, filter to that measured bet:

```bash
lf op pm show --wave product --project wave-chat
```

### `lf op pm task create`

Create a Linear issue in the wave's provider container and attach it to the
local project.

```bash
lf op pm task create --wave product --project wave-chat --title "Retain steward thread after restart"
```

Rules:

- `--project` must name an existing `wave/<wave>/projects/<project>.md`.
- The provider task gets a visible project association.
- If `--project` is omitted, create the task as unassigned and warn.

### `lf op pm task move`

Move or relabel an existing task to a target wave/project:

```bash
lf op pm task move --id <task-id> --wave product --project loopflow-api
```

Rules:

- If the task is already in the target wave's provider container, update only
  the project association.
- If it belongs to another wave's provider container, move it when the provider
  supports moving issues between containers; otherwise clone/link and clearly
  mark the source task as moved.
- Require `--project` to point at an existing local project.

### `lf op pm rename`

Rename the provider container backing a wave:

```bash
lf op pm rename --wave product --title "Product"
```

This is provider-container maintenance, but users should experience it as "make
Linear match this wave."

### `lf op pm sync`

`sync --plan` reads:

- local waves and `pm.*_project` frontmatter
- local project docs
- provider container names
- provider tasks and project associations

Then reports:

- linked provider containers whose display name differs from the local wave
  title
- local waves with no provider container
- provider containers no local wave points to
- tasks with no project association
- tasks whose project association names no local project
- local projects with no open tasks

`sync` applies only low-risk changes:

- rename provider containers to match wave titles
- create missing project associations for known local projects
- label/reassociate tasks when an unambiguous rule exists

Ambiguous task moves stay in the plan output. Do not silently guess.

## Provider mapping

Preferred Linear representation: labels named `project:<slug>`.

Why labels:

- They are visible in Linear.
- They do not require one Linear project per local project.
- They allow a task to stay inside the wave's provider container while
  attaching to a local measured bet.
- Existing tasks can be migrated incrementally.

`lf op pm task create --project wave-chat` creates or ensures the
`project:wave-chat` label and attaches it to the issue.

If Linear supports first-class issue-to-project movement between project
containers, use it for `task move` across waves. If not, implement clone/link
with explicit source-task annotation.

## Immediate migration demo

The new API should demonstrate this branch's exact workflow:

```bash
lf op pm sync --plan
```

Expected plan:

- rename old Mac provider container to Product
- rename old Quality provider container to Intelligence
- rename old Systems provider container to Infrastructure
- report old Datamodel provider container as stranded
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
  provider methods for project create/list and issue create/update/complete.
- `rust/loopflow/src/lfd/pm/linear.rs` already has Linear project create/list
  and issue create/update/complete/comment primitives.
- Add provider methods for:
  - provider-container rename
  - issue label list/create/attach/remove
  - issue project/container move if Linear supports it
  - task lookup by id
  - provider-container listing for stranded detection
- Rename user-facing docs from "roadmap" to "tasks" or "PM tasks"; keep
  "project" reserved for local measured bets.
- Fix the Linear GraphQL ID mismatch while touching this layer: several
  mutations use `String!` where Linear expects `ID!`.

## Done when

### API demos

- `lf op pm status` shows Product, Intelligence, and Infrastructure with
  provider container names and task counts grouped by local project.
- `lf op pm show --wave product` groups tasks by local project and shows
  unassigned tasks separately.
- `lf op pm show --wave product --project wave-chat` shows only Wave Chat
  tasks.
- `lf op pm task create --wave product --project wave-chat --title "Retain steward thread after restart"` creates a Linear issue visibly associated with `project:wave-chat`.
- `lf op pm task move --id <task-id> --wave product --project loopflow-api`
  moves or relabels an existing task and `show --project loopflow-api` displays
  it.
- `lf op pm rename --wave product --title "Product"` renames the Linear
  provider container backing the Product wave.
- `lf op pm sync --plan` reports stranded provider containers, unassigned tasks,
  invalid project associations, and local projects with no tasks.

### Workflow demos from this branch

- Starting from the pre-migration Linear state, `lf op pm sync --plan` identifies
  the old Datamodel provider container as stranded.
- Flowloop work can be moved from the old Datamodel provider container into
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
  "space" as a user-facing noun.
- `docs/wave-authoring.md` says local projects live in
  `wave/<wave>/projects/*.md` and Linear holds tasks.
- No user-facing docs call the Linear provider container a Loopflow project in
  a way that conflicts with measured-bet projects.

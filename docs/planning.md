# Plan and share Tasks

```bash
lf task create --title "Fix the parser" --notes "Quoted input loses escapes"
lf task list
lf capture-tasks          # talk it through; the session files the Tasks
```

Loopflow keeps the plan — Tasks, comments, status — on your machine and syncs it to
one place per repository. That place serves two jobs:

- **Sharing work with coworkers.** Anyone can create, assign, edit, comment on or
  close a Task, and everyone sees the same plan, including what Loopflow is doing
  with it.
- **Capturing work in a meeting.** People talk — a planning meeting or a hallway
  chat — and turn what they decide into Tasks for later, in Loopflow or in the
  tracker they already have open.

Without an external tracker, the plan syncs through your Git remote. With one,
it mirrors into Linear, Asana or Notion. Commands work the same either way, and
every save lands locally first, so nothing waits on the network.

## Without a tracker

```yaml
# .lf/config.yaml — the default; omit it entirely for personal planning
planning:
  provider: git
  remote: origin
```

Your plan travels as a Git ref on the remote, never on a branch. Every machine you
work from fetches and publishes it after each command:

```bash
lf planning status          # what is fetched, pending and published
lf planning key             # this machine's personal planning key
lf planning key --recover <uuid>   # use the same plan on a new machine
```

Share one plan with teammates by naming a shared ref; anyone who can push to the
remote reads and writes it:

```yaml
planning:
  provider: git
  shared: team
```

A personal plan lives at `refs/loopflow/planning/users/<key>`, a shared one at
`refs/loopflow/planning/shared/<name>`. Ref names are not access control; pick a
remote whose readers should see the plan.

**Coworkers** sharing a ref see the same Tasks, Projects, comments and status. When
two people edit the same field, the edit made last wins; the other value stays
in `lf planning status`, ready to copy back.

**Meetings:** capture as you go with `lf task create`, or run `lf capture-tasks`
and describe what was decided; the session drafts the Tasks with you and files them
in their Waves. Teammates see them on their next command.

Only planning travels: checkouts, Sessions, Flows and processes stay on the
machine that ran them.

## With a tracker

Choose one tracker per repository. Repository configuration replaces your personal
default as a whole, so a repository never splits its plan.

```yaml
planning:
  provider: linear
linear:
  team: <team-id>
```

```yaml
planning:
  provider: asana
  project: "<project-gid>"            # the repository's Asana project
```

```yaml
planning:
  provider: notion
  database: "<database-url-or-id>"    # the repository's Notion database
```

```bash
doppler run -- lf account connect linear    # or asana, or notion
```

Each Task is one item in the tracker: a Linear issue, an Asana task, a Notion page.
The tracker is where coworkers work; Loopflow follows them.

| A coworker, in the tracker… | Loopflow… |
|---|---|
| creates a task | adds the Task to `lf task list` |
| renames it or edits its description | updates the Task and tells a running agent |
| comments | delivers the comment to the Task's agent as direction |
| assigns it | records the assignee |
| completes, cancels or reopens it | moves the Task to match |
| deletes it | removes the Task from the plan and keeps its work history |

| You, in Loopflow… | Coworkers see in the tracker… |
|---|---|
| create or capture a Task | a new item, in its Project when it has one |
| edit, comment, complete or reopen | the same change |
| start work on the Task | it moves to in progress |
| open, land or abandon its PR | a link to the PR and its state |

Descriptions and comments keep their formatting: markdown becomes Linear markdown,
Asana rich text or Notion blocks.

**Meetings:** capture where it is easiest. Tasks filed with `lf capture-tasks` or
`lf task create` appear in the tracker; items people add to the tracker during the
meeting appear in `lf task list`.

**Conflicts:** when the same field changes in both places before they sync, the
change sent last wins. If your offline edit is delivered after a coworker's, yours
replaces theirs, and theirs stays in `lf task status` and Desktop as the value it
replaced.

**Offline:** commands save locally and sync on the next command or connected
Session. `lf task sync <task>` retries one Task; a creation whose reply was lost
stays pending until Loopflow finds the item, rather than creating a second one.

### Linear

The repository connects to a Linear Team (`lf repo connect --team-key EXP`). Tasks
take Linear's identifiers, like `EXP-12`. Waves map to Initiatives and each Wave's
Projects to Linear Projects; see [Waves](waves.md).

### Asana

Tasks live in one Asana project. Asana has no task numbers, so Loopflow writes the
Task's identifier into the description's last line. A Wave's Projects appear as
sections of the project. Asana tasks are only open or complete, so work in
progress shows through the PR comment.

### Notion

Tasks are pages in one database; the description is the page body. Loopflow adds a
`Loopflow ID` property to find its pages, and a `Status` property if the database
has none. Status groups carry state: To-do, In progress, Complete, where a Complete
option named Canceled cancels. A `Due` date and `Assignee` person mirror when the
database has them. A Wave's Projects appear as a `Project` select.

Enable **Read comments** and **Insert comments** for the Loopflow integration in
Notion's integration settings; without them, edits still flow but comments do not.

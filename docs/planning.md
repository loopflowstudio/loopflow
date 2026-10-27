# Plan and share Tasks

```bash
lf task create --title "Fix the parser" --notes "Quoted input loses escapes"
lf task list                           # open Tasks in this repository
lf task list loo-60                    # identifier or title contains "loo-60"
lf task list --wave infra --state active
lf task list --done --wave infra --since 7d   # closed in the last week
lf task list --watch                   # a JSON line each time the list changes
lf capture-tasks                       # talk it through; the session files the Tasks
```

`--watch` also works on `lf task status <id>`, `lf wave show`, `lf wave status`,
`lf session list`, `lf history`, `lf planning status`, `lf desktop plans`,
`lf monitor top` and `lf monitor active`, always with `--json`. Each line is
`{"answers", "home", "copy", "revisions", "unavailable", "body"}`, where `body` is
what `--json` prints. Write `{"id": 1, "args": {"query": "parser"}}` to stdin to
change what it reads, or `{"id": 2}` to ask for a fresh reading; the next line
answers that id. It stops when stdin closes. `lf planning status --watch` also
keeps the repository's planning exchanged while it runs.

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

Only planning travels: checkouts, Sessions, Graphs and processes stay on the
machine that ran them.

## With a tracker

Choose one tracker per repository and the team whose plan it joins. Repository
configuration replaces your personal default as a whole, so a repository never
splits its plan.

```yaml
planning:
  provider: asana             # or linear, or notion
  team: "<team-id-or-url>"
```

| Provider | `team` names | Tasks live in |
|---|---|---|
| `linear` | a Linear Team's ID | the Team |
| `asana` | an Asana team's gid or URL | the team project named after the repository |
| `notion` | a Notion page's URL or ID | the page's Tasks database |

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

**Conflicts:** when a title, description, assignee or due date changes in both
places before they sync, the change sent last wins. An edit you saved offline goes
out after a coworker's and replaces it; the tracker's history keeps theirs. Status
follows the tracker: if a coworker reopens a Task you completed offline, it stays
open, and your completion stays in `lf task status` and Desktop.

**Offline:** commands save locally and sync on the next command or connected
Session. `lf task sync <task>` retries one Task; a creation whose reply was lost
stays pending until Loopflow finds the item, rather than creating a second one.

### Linear

The repository connects to a Linear Team (`lf repo connect --team-key EXP`). Tasks
take Linear's identifiers, like `EXP-12`. Waves map to Initiatives and each Wave's
Projects to Linear Projects; see [Waves](waves.md).

Task comments in Linear reach the Task's next useful boundary: before a Graph step,
after an agent turn, when showing planning, and before a Linear write. Team checks
share ten-second freshness. Only Desktop and `--watch` keep a slow clock, backing
off while nothing changes; idle agents do not poll. Teams without live Tasks make
no acquisition requests. A rate limit pauses requests across repositories in the
same Home until reset, retaining pending writes.

### Asana

Every Task lives in the team project named after the repository; Loopflow creates
it the first time if the team has none. A Wave's Projects are Asana projects in the
same team, and a Task in one belongs to both projects. Asana projects take no
outside IDs, so a mirrored project's notes end with a `Loopflow ID:` line; leave it
in place. Asana tasks are only open or complete, so work in progress shows through
the PR comment.

### Notion

The team page holds a **Tasks** database and a **Projects** database, linked by a
`Project` relation, like Notion's Projects & Tasks template. Loopflow uses the ones
already on the page, or creates them. Each Task is a page of Tasks, and its
description is the page body. A Wave's Projects are pages of Projects, and a Task's
`Project` relation names its Project.

Loopflow adds a `Loopflow ID` property to find its pages, plus `Status` and
`Project` when Tasks lacks them. Status groups carry state: To-do, In progress,
Complete, where a Complete option named Canceled cancels. A `Due` date and
`Assignee` person mirror when Tasks has them.

Enable **Read comments** and **Insert comments** for the Loopflow integration in
Notion's integration settings; without them, edits still flow but comments do not.

# lf command reference

```bash
lf help --all
lf help pr land
lf monitor --json
```

Generated from the compiled Clap tree. Canonical command paths are shown; unique shortcuts also resolve. See
[Get started](getting-started.md) for examples. This reference lists public commands.

## Selection and output

`--machine <label-or-id>` runs the command remotely; `--repo NAME_OR_PATH` overrides its saved repository.
`--forward-agent` requires `--machine`.
`--task` selects a Task checkout; `--wt` selects an existing worktree.
`--wave` adds context without moving directories and must match a Task's
owning Wave. `task run` places a Task's worktree and then runs like
`lf --task ISSUE graph GRAPH`; every Graph naming a Task is equally its work.

A preference (`--account`) permits fallback. A restriction (`--only-account`)
limits this launch and its children. A Graph retains its provider selections;
children check their destination's access before starting a provider.
Account observations distinguish unavailable, expired and measured capacity.

JSON readers emit one document; `monitor active --watch --json` emits NDJSON
until stdin closes. Diagnostics go to stderr. Exit 0 means the requested
operation succeeded; 1 denotes an operational failure, 2 a syntax or lookup
failure, and 130 interruption. A successful auto-merge request is not a merge.

## Graph decisions and recovery

```bash
lf task run EXP-12 pursue
lf graph show GRAPH_ID --processes --json
lf task interrupt EXP-12
```

Each start is a new Graph: one lf process and the step processes it starts. Its ID
is the Graph process's. `--state` selects `current` (the Graph process has no recorded
exit), `completed` (it succeeded) or `stopped` (it exited before the last
step). A stopped Graph's Processes are its history; nothing resumes it. Inspect
them, then launch the work that remains.

## lf

Open Loopflow or run its CLI

| Argument | What it does |
|---|---|
| `--context` | Preview assembled agent input without launching or changing Work Default: false. |
| `--explain` | Explain this invocation's Work selection without executing it Default: false. |
| `--json` | Emit invocation previews as JSON Default: false. |
| `--docs` | Docs paths, globs, or directories to include in context |
| `--clipboard / -c` | Include clipboard content in prompt Default: false. |
| `--agent / -a` | Override agents named in Tasks, Graphs and skills, including child commands (claude, codex, opencode, pi; harness[:model]) Applies everywhere. |
| `--skill-input` | Read the skill and its arguments from this captured invocation file, as a Graph step does |
| `--output-schema` | Require the final answer to match this JSON schema, as a deciding Graph step does |
| `--account` | Prefer this managed provider login before the normal route. Repeat to select provider-qualified preferences such as `claude=jack@`. Logins spend; a profile is only the Chrome venue accounts log in through, so it is never a run-time selector |
| `--only-account` | Restrict this invocation and its children to exactly these managed provider logins. Providers without a selection are unavailable |
| `--isolate` | Run in the selected account's own provider home, unmoved by account switches. Applies to this invocation and its children Default: false. |
| `--shared` | Run in the provider's ordinary home despite an `isolate: true` default Default: false. |
| `--yolo` | Skip permission prompts Default: false. |
| `--interactive / -i` | Run interactively Default: false. |
| `--batch / -b` | Run headless: print the output and return when the work ends Default: false. |
| `--verbose / -v` | Show context accounting and diagnostic logs Applies everywhere. Default: false. |
| `--chrome` | Override Chrome integration; omission inherits configuration |
| `--diff` | Select changed-code context; omission inherits configuration |
| `--machine` | Run the command on this saved machine in its repository |
| `--repo` | Select a repository by name under repo_root (default ~/src), or by explicit path |
| `--repository` | Select a registered repository plan on this Machine, not a clone name |
| `--forward-agent` | Forward the SSH agent to the selected machine Default: false. |
| `--wave` | Add Wave context and identity without changing the working directory |
| `--task` | Execute in this Task's checkout |
| `--wt` | Execute in an existing worktree by name or branch |
| `--cwd` | Run in this directory; Work still comes from `--task` or `--wave` |
| `--help / -h` | Print help |
| `--version / -V` | Print version |

## lf task

Concrete work and Task lifecycle

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf task list

List every Task in the selected repository

| Argument | What it does |
|---|---|
| `--wave` | wave |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task defer

Defer future Task execution without stopping live processes

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--help / -h` | Print help |

## lf task resume

Resume a deferred Task without starting execution

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--help / -h` | Print help |

## lf task sync

Synchronize a Task with its tracker; the change sent last wins

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--retry-creation` | Retry an uncertain issue creation with its saved ID; may recreate a deleted issue Default: false. |
| `--retry-updates` | Retry uncertain saved field/state updates after observing current Linear values Default: false. |
| `--help / -h` | Print help |

## lf task checkout

Ensure tracked Task Work and its worktree without launching a Graph

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--name` | name |
| `--stack-on` | Fork this Task's worktree from another Task's active PR |
| `--directive` | directive |
| `--design` | Hand off a child-specific design from the caller's checkout |
| `--recover-from` | Restore a missing Task branch from a deliberately recovered commit; never reset existing work |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task run

Run the Task's Graph in its worktree until the next interactive step; returns once it runs (`-b` runs every step headless and waits for the end)

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `<graph>` | A graph to take up in place of the Task's own |
| `--from` | Start at this step instead: to go back, skip ahead, or wait at an interactive step |
| `--name` | name |
| `--stack-on` | Fork this Task's worktree from another Task's active PR |
| `--directive` | directive |
| `--reason` | Direction for this run, published to the Task |
| `--pool` | Run on a free machine you own in this pool |
| `--rent` | Rent a machine from this on-demand (after `--pool`, only when none is free) |
| `--quote` | Print where it would run and the most it could cost; run nothing Default: false. |
| `--help / -h` | Print help |

## lf task complete

Complete a Task without moving its Graph

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--reason` | Why, kept in the Task's completion request |
| `--help / -h` | Print help |

## lf task reopen

Reopen local planning without moving its Graph or replacing the PR

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--reason` | reason |
| `--help / -h` | Print help |

## lf task create

Create a planning Task; with --run, start it and open its conversation

| Argument | What it does |
|---|---|
| `<request>` | What you want, in your words: the Task's notes, its first line the title. With --run, also the first turn of its conversation |
| `--no-wave` | Create without a Wave (the default) Default: false. |
| `--wave` | Explicit Wave membership; omitted creates a standalone Task |
| `--title` | Task title; omitted when stdin supplies the report and first line |
| `--notes` | Description; defaults to a report read from stdin |
| `--run` | Start it as `lf task run` does, then open its conversation here Default: false. |
| `--graph` | The graph it takes up in place of the default |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task claim

Claim the Task for you on this Machine

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task unclaim

Unclaim the Task; it is unowned again, with its history intact

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task location

Read recorded execution location without preparing or transferring Work

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--peers` | Observe added Machines as well as this one Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task shell

Open a shell in an existing Task checkout, without preparing work

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--checkout` | checkout |
| `--help / -h` | Print help |

## lf task watch-files

Stream invalidations from an existing Task checkout (JSON lines)

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--checkout` | checkout |
| `--request` | request |
| `--help / -h` | Print help |

## lf task status

Show durable Task facts and its recorded work

| Argument | What it does |
|---|---|
| `<issue>` | Task issue; defaults to the Task in this checkout |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task diff

Show this Task's patch or list its changed files

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `<path>` | path |
| `--files` | List changed paths and comparison revisions instead of a patch Default: false. |
| `--base` | base Default: parent. |
| `--draft` | Compare a UTF-8 draft read from stdin without writing the worktree Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task files

List one directory in this Task's worktree

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `<directory>` | directory Default: .. |
| `--cursor` | cursor |
| `--show-ignored` | show ignored Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task file

Read one file from the Task checkout

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `<path>` | path |
| `--recoveries` | Inspect retained versions, including late writes; omitted for fast content reads Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task save

Save UTF-8 stdin with an expected revision and retained recovery files

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `<path>` | path |
| `--revision` | revision |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task abandon

Cancel the Task in Linear and locally, close its PRs and delete its branches

| Argument | What it does |
|---|---|
| `<issue>` | Issue ID or branch; defaults to the Task in this checkout |
| `--force / -f` | force Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task sweep

Preview open issues outside current chapters; apply safe cancellations explicitly

| Argument | What it does |
|---|---|
| `--apply` | apply Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task delete

Cancel unfinished placed work, clean up delivery, then trash the Linear issue

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--help / -h` | Print help |

## lf task edit

Edit a Task's title or notes, before or after placement

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--title` | title |
| `--notes` | notes |
| `--rank` | Zero-based position in the Project's Task order |
| `--assignee` | Assignee identity; provider user ID in connected plans |
| `--unassign` | unassign Default: false. |
| `--wave / -w` | wave |
| `--help / -h` | Print help |

## lf task move

Change Task membership, or move the Task to another Machine, preserving its recorded work

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--wave / -w` | wave |
| `--no-wave` | no wave Default: false. |
| `--machine` | Move the Task to this Machine, by label or id |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task comment

Read the thread or publish a comment; agent comments default to progress

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `<message>` | message |
| `--steer` | Save a steer even when commenting from an agent Session Default: false. |
| `--wave / -w` | wave |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task interrupt

Stop the Task's running Graph or Task run and its scheduled automation

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf task wait

Wait without polling an LM

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--until` | until Default: terminal. |
| `--timeout` | timeout |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf wave

Manage Wave identity and planning

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf wave show

Read chapter plans and Tasks without starting work; an explicit Wave includes history

| Argument | What it does |
|---|---|
| `<wave>` | wave |
| `--task` | Find an exact Task, including historical planning |
| `--json` | json Default: false. |
| `--all` | Read every repository on this Machine Default: false. |
| `--help / -h` | Print help |

## lf wave graph

Select and inspect the graph the Wave's Tasks take up by default

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf wave graph show

Show the graph the Wave's Tasks take up by default

| Argument | What it does |
|---|---|
| `<wave>` | wave |
| `--no-wave` | Show the default for Tasks filed in the repository, without a Wave Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf wave graph set

Select the graph future Tasks take up; Tasks that took one up keep it. Graphs are committed `.lf/graphs/<name>.yaml` files or builtins

| Argument | What it does |
|---|---|
| `<name>` | name |
| `<wave>` | wave |
| `--help / -h` | Print help |

## lf wave edit-plan

Edit the selected chapter's name or summary

| Argument | What it does |
|---|---|
| `<wave>` | Wave name or ID (default: the ambient Wave) |
| `--project` | Exact retained Project ID, provider ID or unique name instead of current selection |
| `--name` | name |
| `--summary` | summary |
| `--description` | Authored chapter prose |
| `--help / -h` | Print help |

## lf wave new-chapter

Rotate this Wave, or every Wave with --all, to the named Project plan

| Argument | What it does |
|---|---|
| `<wave>` | wave |
| `<name>` | name |
| `--all` | Advance every Wave in this repository Default: false. |
| `--plan` | plan |
| `--dry-run` | dry run Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf wave ensure

Ensure the configured Project is active, or create one with a durable identity

| Argument | What it does |
|---|---|
| `<wave>` | wave |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf wave bind-project

Bind an existing Project UUID as this Wave's shared current selection

| Argument | What it does |
|---|---|
| `<wave>` | wave |
| `<project>` | project |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf wave list

List authored Waves and retained planning identities without starting work

| Argument | What it does |
|---|---|
| `--json` | Emit the wave snapshot as JSON (Loopflow's dashboard snapshot) Default: false. |
| `--all` | List Waves from every repository on this machine, not just the current repository (worktrees collapse to their main checkout) Default: false. |
| `--current` | Exclude abandoned and retired registrations from current navigation Default: false. |
| `--help / -h` | Print help |

## lf wave status

Show one Wave's current plan, Task details, and execution evidence

| Argument | What it does |
|---|---|
| `<wave>` | Wave name (default: the ambient wave) |
| `--json` | Emit the status snapshot as JSON Default: false. |
| `--sync` | Refresh planning from its provider (Git or Linear) before reading Default: false. |
| `--help / -h` | Print help |

## lf wave rename

Rename or relocate an authored Wave and its provider mapping

| Argument | What it does |
|---|---|
| `<wave>` | wave |
| `--repo` | repo |
| `--name` | name |
| `--title` | Change the linked Initiative display title |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf wave update-plan

Replace the current chapter's KRs and targets

| Argument | What it does |
|---|---|
| `--wave / -w` | wave |
| `--plan` | The complete plan as JSON |
| `--help / -h` | Print help |

## lf wave connect

Connect a Wave to its Initiative and the repository's Team (Task prefix)

| Argument | What it does |
|---|---|
| `<wave>` | Wave name (auto-detected if omitted) |
| `--all` | Recursively initialize every Wave under .lf/wave/ Default: false. |
| `--team-key` | Repository Team key = Task prefix (e.g. LOO). Defaults from the repository name |
| `--team-name` | Repository Team display name. Defaults to the repository name |
| `--help / -h` | Print help |

## lf wave reteam

Reconcile linked Waves to the repository's Linear Team

| Argument | What it does |
|---|---|
| `--apply` | apply Default: false. |
| `--help / -h` | Print help |

## lf ship

Ship the Task: run every remaining step to merge and completion, standing in at each interactive step

| Argument | What it does |
|---|---|
| `<issue>` | issue |
| `--graph` | Take up this graph in place of the Task's remaining steps |
| `--reason` | Why, kept in the Task's history |
| `--stop` | Stop Auto-Ship: end the ship process and disarm auto-merge it armed Default: false. |
| `--help / -h` | Print help (see more with '--help') |

## lf session

Inspect and continue Sessions

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf session serve

Run the process a Session lives in while terminals come and go

| Argument | What it does |
|---|---|
| `--terminal` | Relay the execution to an attached terminal Default: false. |
| `<argv>` | The command this process executes |
| `--help / -h` | Print help |

## lf session attach

Attach to the current execution without restarting or submitting input

| Argument | What it does |
|---|---|
| `<id>` | id |
| `--observe` | observe Default: false. |
| `--if-free` | Refuse to replace another viewer's input control Default: false. |
| `--prepare` | Return an attach-only terminal command without acquiring control Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf session steer

Save a steer for this Session, or without one for the current Wave's or repository's Sessions, without starting a turn

| Argument | What it does |
|---|---|
| `<id>` | id |
| `<message>` | message |
| `--help / -h` | Print help |

## lf session steers

Inspect steer delivery or retry delivery

| Argument | What it does |
|---|---|
| `<id>` | id |
| `--json` | json Default: false. |
| `--retry` | retry |
| `--after` | after Default: 0. |
| `--limit` | limit Default: 32. |
| `--source` | Read the complete steer |
| `--help / -h` | Print help |

## lf session observe-status

Observe validated Program Status snapshots from an active local terminal

| Argument | What it does |
|---|---|
| `<id>` | id |
| `--terminal` | The terminal marker in the current provider client receipt |
| `--agent-process` | AgentProcess from the Session reading; omit when it reported none |
| `--help / -h` | Print help |

## lf session history

Read this conversation's native start, usage and completion receipts

| Argument | What it does |
|---|---|
| `<id>` | Session ID or provider conversation ID |
| `--json` | json Default: false. |
| `--after` | Continue after an observed event sequence Default: 0. |
| `--limit` | limit Default: 100. |
| `--help / -h` | Print help |

## lf session list

List Sessions

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--all` | Include Sessions from every repository on this machine Default: false. |
| `--interactive` | Select interactive (true), headless (false), or both (all) Default: true. |
| `--history` | Include completed conversations and historical reviews Default: false. |
| `--available` | Only conversations available for your input Default: false. |
| `--limit` | Maximum conversations; 0 reads the complete matching inventory Default: 100. |
| `--offset` | offset Default: 0. |
| `--page` | Return a bounded stable-ID page with a continuation cursor Default: false. |
| `--after` | Previous page's next identity; keep the same filters |
| `--task` | task |
| `--orphan` | Only Sessions without a Task association Default: false. |
| `--search` | search |
| `--help / -h` | Print help |

## lf session connect

Open the ongoing conversation of this scope, or an exact Session

| Argument | What it does |
|---|---|
| `<id>` | Session ID, Task identifier, or provider conversation ID |
| `--task` | task |
| `--wave` | wave |
| `--fresh` | Create a fresh primary in this scope, keeping the previous client and history Default: false. |
| `--message` | With `-b`: send the conversation its next turn |
| `--prepare` | Return connection data without starting the provider Default: false. |
| `--json` | json Default: false. |
| `--takeover` | Stop Loopflow-owned clients before resuming here Default: false. |
| `--try` | Ask the provider to resume even when another client is active Default: false. |
| `--all` | Resume every conversation a restart or crash interrupted; never one someone left. Terminals attach when they open them Default: false. |
| `--help / -h` | Print help |

## lf session rename

Rename a Session; a human name is never replaced by a suggestion

| Argument | What it does |
|---|---|
| `<id>` | Session ID or provider conversation ID |
| `<name>` | name |
| `--suggest` | Propose an agent-generated name; keeps a human-assigned name Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf session bind

Assign a Task to a Session that has none; the Task never changes after

| Argument | What it does |
|---|---|
| `<id>` | Session ID or provider conversation ID |
| `--task` | The Task, by its issue identifier (e.g. INF-123) or stable Task ID |
| `--dry-run` | Resolve the exact target without assigning the Session Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf session send

Hand a Session to another Machine: commit and push its branch, stop it here and print the move for `lf session receive` there

| Argument | What it does |
|---|---|
| `<id>` | Session ID or provider conversation ID |
| `--task` | Send this Task with its Session, as `lf task move --machine` does |
| `--to` | The Machine it moves to, by label or id |
| `--help / -h` | Print help |

## lf session receive

Admit a Session `lf session send` printed, read from stdin: check out its branch and place its conversation so it resumes here

| Argument | What it does |
|---|---|
| `--check` | Read no move; exit non-zero unless this Machine can receive one Default: false. |
| `--task` | With --check, also require that this Machine knows the Task |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf open

Open Work's conversation in Desktop, this cmux/Herdr host or a terminal

| Argument | What it does |
|---|---|
| `<url>` | Work address, such as loopflow://loopflow/task/LOO-507 |
| `--host` | Presentation destination; auto opens in this Herdr/tmux/cmux host, then Desktop, then a terminal Default: auto. |
| `--available` | Open every conversation available for you to work with Default: false. |
| `--session` | Open this exact Session rather than its Work's primary conversation |
| `--diff` | Show the Task's Changes browser beside its conversation (Desktop) Default: false. |
| `--link` | Print the readable link without opening, preparing or creating anything Default: false. |
| `--format` | Link text format; text uses a clickable span on a terminal |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf :

Run an inline prompt

| Argument | What it does |
|---|---|
| `<prompt>` | prompt |
| `--help / -h` | Print help |

## lf graph

Run or inspect authored graphs

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf graph list

List authored graphs, or Graphs that ran

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--processes` | List or show Graphs that ran, from their Processes, instead of reusable templates Default: false. |
| `--all` | Include every repository and graphs with unknown repository evidence Default: false. |
| `--limit` | Page size (default 100) |
| `--after` | Previous page's next identity; retain the same filters |
| `--search` | Literal name or identity containment |
| `--state` | state |
| `--for-task` | Retained Task ID or issue identifier, including completed Tasks |
| `--for-wave` | Retained Wave ID or name |
| `--taskless` | taskless Default: false. |
| `--help / -h` | Print help |

## lf graph show

Inspect an authored graph, or one that ran

| Argument | What it does |
|---|---|
| `<name>` | name |
| `--json` | json Default: false. |
| `--processes` | processes Default: false. |
| `--help / -h` | Print help |

## lf graph customize

Print the repository file that defines a Graph, creating it from the builtin when the repository has none

| Argument | What it does |
|---|---|
| `<name>` | name |
| `--help / -h` | Print help |

## lf skill

Run a skill explicitly

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf skill customize

Print the repository file that defines a skill, creating it from the builtin when the repository has none

| Argument | What it does |
|---|---|
| `<name>` | name |
| `--help / -h` | Print help |

## lf history

Read durable Work history, recorded commands and provider usage

| Argument | What it does |
|---|---|
| `--since` | Relative window (7d, 24h, 30m) or RFC3339 start Default: 7d. |
| `--limit` | Maximum rows after Work filters (1-200) Default: 50. |
| `--wave` | Scope to one Wave by name |
| `--project` | Scope to one Project by slug |
| `--task` | Scope to one Task by Linear identifier |
| `--json` | Emit the typed activity snapshot as JSON Default: false. |
| `--help / -h` | Print help |

## lf history list

List a bounded page of recorded commands, newest first

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--all` | Include all repositories Default: false. |
| `--limit` | limit Default: 100. |
| `--after` | Continue with the previous page's next object, encoded as JSON |
| `--parent` | Direct children of an exact or unambiguous parent Process |
| `--caller` | Commands issued by this Session |
| `--search` | Literal command text, ignoring ASCII case |
| `--outcome` | outcome |
| `--task` | Recorded work for a Task, including completed Tasks |
| `--wave` | Recorded work for a Wave |
| `--help / -h` | Print help |

## lf history show

Inspect a process or Session by identity

| Argument | What it does |
|---|---|
| `<id>` | id |
| `--json` | json Default: false. |
| `--final` | Print a Session's last recorded provider conclusion Default: false. |
| `--input` | Inspect an exact retained input belonging to this Session |
| `--context` | Print only what the step's submitted input was made of, by source Default: false. |
| `--instructions` | Explain recorded instruction inclusion and omission Default: false. |
| `--transcript` | Print only the Session's conversation, as its providers kept it Default: false. |
| `--help / -h` | Print help |

## lf history retention

Show how much Session history providers still keep, and what they remove next

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf history search

Find text in the conversations of recent Sessions

| Argument | What it does |
|---|---|
| `<text>` | text |
| `--since` | Relative window (7d, 24h, 30m) or RFC3339 start Default: 7d. |
| `--wave` | Scope to one Wave by name |
| `--project` | Scope to one Project by slug |
| `--task` | Scope to one Task by Linear identifier |
| `--limit` | limit Default: 50. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf history usage

Show direct provider-authored usage from recorded Session inputs

| Argument | What it does |
|---|---|
| `--json` | Emit Session usage evidence as JSON Default: false. |
| `--days` | Observation window, in days (zero means all time) Default: 30. |
| `--weekly` | Report context cost and turn time by week since 2026-09-30 Default: false. |
| `--binds` | Compare Task and Wave usage under prospective and post-hoc bind attribution Default: false. |
| `--parent` | Inputs issued by this Session or retained capture |
| `--wave` | Limit to Session inputs attributed to one Wave |
| `--project` | Limit to Session inputs attributed to one Project |
| `--task` | Limit to Session inputs attributed to one Task |
| `--context` | Break each step's submitted input down by source, flagged against budgets Default: false. |
| `--help / -h` | Print help |

## lf history replay

Launch a new execution from a captured provider request, while this Machine retains its capture

| Argument | What it does |
|---|---|
| `<run>` | Captured input identity or an unambiguous displayed prefix |
| `--help / -h` | Print help |

## lf history export

Write this Machine's Sessions, Processes and captures as JSON lines, for the Machine that copies them before giving this one back

| Argument | What it does |
|---|---|
| `--since` | Relative window (7d, 24h, 30m) or RFC3339 start |
| `--help / -h` | Print help |

## lf desktop

Select Work and control Loopflow Desktop panes

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf desktop view

Show a Work selection, or derive a passive plan for external hosts

| Argument | What it does |
|---|---|
| `<expression>` | wave/task/query/kind/state expression; omitted uses the repository default |
| `--plan` | Print the derived plan as JSON without opening Desktop or starting a provider Default: false. |
| `--json` | Print the opening receipt as JSON Default: false. |
| `--help / -h` | Print help |

## lf desktop query

Create or edit a Dashboard query with revision checking; prints the Dashboard as JSON

| Argument | What it does |
|---|---|
| `<container>` | Dashboard name or query:ID |
| `<expression>` | Selection expression, as for `lf desktop view` |
| `--revision` | Last observed revision; omit only when creating a new Dashboard |
| `--help / -h` | Print help |

## lf desktop pin

Pin a Dashboard in repository discovery; prints the Dashboard as JSON

| Argument | What it does |
|---|---|
| `<query>` | Dashboard name or query:ID |
| `--revision` | The Dashboard's last read revision; a stale one writes nothing |
| `--help / -h` | Print help |

## lf desktop unpin

Unpin a Dashboard without changing its membership or arrangement; prints the Dashboard as JSON

| Argument | What it does |
|---|---|
| `<query>` | Dashboard name or query:ID |
| `--revision` | The Dashboard's last read revision; a stale one writes nothing |
| `--help / -h` | Print help |

## lf desktop rename

Rename a Dashboard or Task through its shared owner; prints the owner's record as JSON

| Argument | What it does |
|---|---|
| `<target>` | query:ID or task:<durable-id> |
| `<name>` | The new name, written to the shared owner so labels and crumbs follow |
| `--revision` | The target's last read revision; a stale one writes nothing |
| `--help / -h` | Print help |

## lf desktop layout

Read a Work's pane intent or a Dashboard's slots as JSON, or save a complete revision-checked record from stdin

| Argument | What it does |
|---|---|
| `<work>` | repository, wave:<durable-id>, task:<durable-id> or query:ID |
| `--save` | Replace the record from JSON on stdin instead of reading it (a Dashboard's: slots, null for the default) Default: false. |
| `--revision` | A Dashboard's last read revision; a stale one writes nothing |
| `--help / -h` | Print help |

## lf desktop ls

List the window's Tasks by position (as ⌘1–9 select them) and each Task's panes

| Argument | What it does |
|---|---|
| `--window` | Desktop window: ID, position or ID suffix (default: this terminal's window) |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop state

Read actual windows and panes without launching, focusing, or acquiring clients

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop actions

List the verbs Desktop performs, with their arguments and default keys

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop events

Stream window, pane and destination changes; the first change is every window

| Argument | What it does |
|---|---|
| `--json` | One change per line as JSON Default: false. |
| `--help / -h` | Print help |

## lf desktop wait

Wait for a shell tab's command to finish (exits with its status), else for the pane to appear

| Argument | What it does |
|---|---|
| `--for` | Longest wait, e.g. 30s, 5m (default: no limit) |
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop back

Go back, as ⌘[ and the crumb chevron do

| Argument | What it does |
|---|---|
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop forward

Go forward, as ⌘] does

| Argument | What it does |
|---|---|
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop goto

Show the window's Nth Working Task, as ⌘N does

| Argument | What it does |
|---|---|
| `<index>` | index |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop next

Show the next Working Task, as ⌃⇥ does

| Argument | What it does |
|---|---|
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop previous

Show the previous Working Task, as ⌃⇧⇥ does

| Argument | What it does |
|---|---|
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop working-tasks

Show the Working Tasks Dashboard

| Argument | What it does |
|---|---|
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop enter

Enter a member of the shown Dashboard as full Work, as a tile click does

| Argument | What it does |
|---|---|
| `<tile>` | Tile on the shown Dashboard or Wave view, as `-p` names it: position, ID, label or ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop do

Perform any registry action by name, as `lf desktop actions` lists them

| Argument | What it does |
|---|---|
| `<action>` | Action name from `lf desktop actions` |
| `<args>` | Action arguments as KEY=VALUE |
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop send

Insert literal text at a terminal's cursor (stdin when omitted); never submits

| Argument | What it does |
|---|---|
| `<text>` | Unicode text without control characters (use send-key for Enter/Tab) |
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop send-key

Send one key to a terminal, without following focus

| Argument | What it does |
|---|---|
| `<key>` | key |
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop capture

Print a terminal's text, without following focus

| Argument | What it does |
|---|---|
| `--region` | region Default: screen. |
| `--trim` | Drop trailing spaces and blank lines Default: false. |
| `--max-bytes` | max bytes Default: 65536. |
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop run

Add a shell tab at the Task's location and type LINE into it, as if typed; focus stays put

| Argument | What it does |
|---|---|
| `<line>` | One shell line, e.g. 'cargo test -p loopflow'; omitted opens an empty shell |
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop zoom

Zoom a pane, or return it to the layout, without changing selection

| Argument | What it does |
|---|---|
| `--on` | on Default: false. |
| `--off` | off Default: false. |
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop focus

Focus a pane, or the pane in a direction, opening its retained workspace when needed

| Argument | What it does |
|---|---|
| `--direction` | direction |
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop hide

Hide a pane without closing its Session or process; on a Dashboard, hide a tile from its shared record

| Argument | What it does |
|---|---|
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop show

Show a hidden pane without selecting Work or acquiring a client; on a Dashboard, a hidden tile

| Argument | What it does |
|---|---|
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop close

Close a shell or browser tab, as ⌘W does; a session pane hides

| Argument | What it does |
|---|---|
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop reopen

Undo the Task's last close, as ⇧⌘T does

| Argument | What it does |
|---|---|
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop move

Move a pane beside another pane of the same Task; on a Dashboard or Wave view, a tile into another's place

| Argument | What it does |
|---|---|
| `--destination` | Pane to place it beside, or tile whose place it takes, as for -p |
| `--axis` | Split axis for a pane; tiles take the destination's place |
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop resize

Resize the divider between two panes or Dashboard tiles; ratio is the addressed one's share

| Argument | What it does |
|---|---|
| `--toward` | The other pane or tile, as for -p |
| `--ratio` | ratio |
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop files

Show the Task's Files in the browser

| Argument | What it does |
|---|---|
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop graph-log

Show the Task's Graph processes in the browser

| Argument | What it does |
|---|---|
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop browse

Open an http(s) page in a browser tab

| Argument | What it does |
|---|---|
| `<url>` | url |
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop page-back

Go back in a web page

| Argument | What it does |
|---|---|
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop page-forward

Go forward in a web page

| Argument | What it does |
|---|---|
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop reload

Reload a web page, as ⌘R does

| Argument | What it does |
|---|---|
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf desktop switch

Show another of the Task's conversations in its session pane

| Argument | What it does |
|---|---|
| `<session>` | Session ID |
| `--pane / -p` | Pane path: a view part (board), session, shell:2, a tile (3, LOO-519) or TILE/PANE; each part resolves by ID, label, position, ID suffix |
| `--window` | Desktop window: ID, position or ID suffix |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf monitor

Show waiting, blocked, active, and finished work with next actions

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--all` | all Default: false. |
| `--help / -h` | Print help |

## lf monitor active

Observe active conversations and missing process evidence

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--watch` | Stream NDJSON until stdin closes Default: false. |
| `--task` | task |
| `--help / -h` | Print help |

## lf monitor work

Stream planning and activity for the selected Work, each part again only when it changes

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--watch` | Stream NDJSON until stdin closes Default: false. |
| `--help / -h` | Print help |

## lf monitor ps

Print one parseable snapshot of live Loopflow call trees

| Argument | What it does |
|---|---|
| `--json` | Emit the versioned activity snapshot as JSON Default: false. |
| `--help / -h` | Print help |

## lf monitor top

Refresh live Loopflow call trees on a terminal; print once when redirected

| Argument | What it does |
|---|---|
| `--json` | Emit one versioned activity snapshot as JSON Default: false. |
| `--help / -h` | Print help |

## lf monitor rollover

Move idle Sessions still running a superseded lf onto the selected install

| Argument | What it does |
|---|---|
| `--dry-run` | List what would move and what stays without stopping anything Default: false. |
| `--json` | Emit the rollover report as JSON Default: false. |
| `--help / -h` | Print help |

## lf monitor prune

Reap orphan providers, remove dead receipts, forget finished Process rows and release unrecorded AgentProcess ancestry

| Argument | What it does |
|---|---|
| `--dry-run` | Show exact targets without changing processes, receipts or the store Default: false. |
| `--json` | Emit the versioned prune report as JSON Default: false. |
| `--help / -h` | Print help |

## lf ops

Software delivery and the environment work runs in: PRs, CI, releases, commits, worktrees, Machines and provider accounts

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf ops pr

Pull request lifecycle

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf ops pr reconcile

Check recorded repository landings once, record CI failures, and settle verified merges

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf ops pr checks

Show CI status for current branch

| Argument | What it does |
|---|---|
| `--watch / -w` | watch Default: false. |
| `--logs / -l` | logs Default: false. |
| `--required` | Show only checks required by repository protection Default: false. |
| `--help / -h` | Print help |

## lf ops pr request-review

Request or re-request a teammate's review of an existing PR

| Argument | What it does |
|---|---|
| `<pr>` | PR number, URL or branch; defaults to the current branch's PR |
| `--reviewer / -r` | GitHub login or org/team. Repeat to request several reviewers |
| `--help / -h` | Print help |

## lf ops pr reviews

Read requested reviews and review submissions from GitHub

| Argument | What it does |
|---|---|
| `<pr>` | PR number, URL or branch; defaults to the current branch's PR |
| `--json` | Emit GitHub CLI JSON, including each review's commit and body Default: false. |
| `--help / -h` | Print help |

## lf ops pr publish

Publish a ready PR headlessly: push, create or refresh, print state + URL. Opens no review surface

| Argument | What it does |
|---|---|
| `--title` | title |
| `--body` | body |
| `--help / -h` | Print help |

## lf ops pr open

Push and create or update a draft PR, then open its GitHub page. Existing ready PRs stay ready; opening a draft does not publish it

| Argument | What it does |
|---|---|
| `--title` | title |
| `--body` | body |
| `--help / -h` | Print help |

## lf ops pr submit

Prepare a PR to land: sync, clear committed scratch additions and mark ready. Leave it for manual merge without changing reviewers or assignees

| Argument | What it does |
|---|---|
| `--strict` | strict Default: false. |
| `--worktree / -w` | worktree |
| `--message / -m` | message |
| `--title` | title |
| `--body` | body |
| `--help / -h` | Print help |

## lf ops pr arm

Prepare a PR, request exact-head auto-merge, and return without watching

| Argument | What it does |
|---|---|
| `--strict` | strict Default: false. |
| `--local` | local Default: false. |
| `--worktree / -w` | worktree |
| `--message / -m` | message |
| `--title` | title |
| `--body` | body |
| `--help / -h` | Print help |

## lf ops pr land

Request auto-merge, retain settlement intent, and return

| Argument | What it does |
|---|---|
| `--wait-and-fix` | Wait for verified merge and repair failing CI until interrupted or blocked Default: false. |
| `--strict` | strict Default: false. |
| `--local` | local Default: false. |
| `--worktree / -w` | worktree |
| `--message / -m` | message |
| `--title` | title |
| `--body` | body |
| `--help / -h` | Print help |

## lf ops pr abandon

Abandon branch: close PR, remove worktree, delete branch

| Argument | What it does |
|---|---|
| `<branch>` | Branch to abandon (default: current) |
| `--force / -f` | force Default: false. |
| `--help / -h` | Print help |

## lf ops release

Release operations (run, check, notes, bump, tag, status)

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf ops release run

Run the full release workflow end-to-end

| Argument | What it does |
|---|---|
| `<version>` | Version to release: patch\|minor\|major\|X.Y.Z (default: patch) |
| `--target / -t` | target |
| `--help / -h` | Print help |

## lf ops release check

Check if PRs have merged since the last tag

| Argument | What it does |
|---|---|
| `--target / -t` | target |
| `--help / -h` | Print help |

## lf ops release notes

Generate release notes for a version

| Argument | What it does |
|---|---|
| `<version>` | Version (e.g. 0.9.6) |
| `--prev-tag` | prev tag |
| `--preview` | Print notes without updating manifests or release archives Default: false. |
| `--target / -t` | target |
| `--help / -h` | Print help |

## lf ops release bump

Bump version in manifest files

| Argument | What it does |
|---|---|
| `<version>` | Version to bump to (e.g. 0.9.6) |
| `--target / -t` | target |
| `--help / -h` | Print help |

## lf ops release tag

Create a git tag and push it

| Argument | What it does |
|---|---|
| `<version>` | Version to tag (e.g. 0.9.6) |
| `--target / -t` | target |
| `--help / -h` | Print help |

## lf ops release publish

Stage or publish a GitHub Release

| Argument | What it does |
|---|---|
| `<tag>` | Release tag (for example v0.12.4) |
| `--notes` | Release notes used while creating or updating the draft |
| `--asset` | Asset to upload; repeat for multiple files |
| `--finalize` | Publish the existing draft and mark it latest Default: false. |
| `--help / -h` | Print help |

## lf ops release status

Check release workflow status

| Argument | What it does |
|---|---|
| `--target / -t` | target |
| `--help / -h` | Print help |

## lf ops ci

Show how failed CI is detected, repaired, and landed across this Machine

| Argument | What it does |
|---|---|
| `--since` | Relative window (7d, 24h, 30m) or RFC3339 start Default: 7d. |
| `--wave` | Scope to one Wave |
| `--repo` | Scope to one GitHub owner/repo |
| `--json` | Emit the complete incident report as JSON Default: false. |
| `--help / -h` | Print help |

## lf ops commit

Commit changes

| Argument | What it does |
|---|---|
| `--message / -m` | message |
| `--push / -p` | Push the branch after committing Default: false. |
| `--no-add` | no add Default: false. |
| `<paths>` | Commit only these paths, preserving other staged and unstaged edits |
| `--help / -h` | Print help |

## lf ops sync

Merge upstream into the current branch (default: main or stack parent)

| Argument | What it does |
|---|---|
| `--plan` | Print the planned sync strategy without mutating git Default: false. |
| `--manual` | Keep the sync and uncommitted edits local; leave conflicts for this process to resolve Default: false. |
| `--continue` | Stage resolved conflict paths and continue the local sync Default: false. |
| `--abort` | Abort the local sync in progress Default: false. |
| `--adopt` | Explicitly claim a raw sync that has no Loopflow owner Default: false. |
| `<onto>` | Branch to sync onto |
| `--help / -h` | Print help |

## lf ops wt

Worktree operations

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf ops wt create

Create a low-level worktree following the repository's existing layout

| Argument | What it does |
|---|---|
| `<name>` | Worktree name |
| `--plan` | Print the placement plan without creating a worktree Default: false. |
| `--persistent` | Keep this workspace after delivery Default: false. |
| `--help / -h` | Print help |

## lf ops wt switch

Switch to a worktree by name, identity leaf, or full branch

| Argument | What it does |
|---|---|
| `<name>` | Worktree name or full branch name to switch to |
| `--help / -h` | Print help |

## lf ops wt list

List worktrees (read-only; reflects the last-synced main)

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--sync` | Fetch origin and fast-forward main before listing (mutates the canonical checkout). Off by default so a list never touches it Default: false. |
| `--help / -h` | Print help |

## lf ops wt timing

Report how long `lf wt list` has taken on this machine

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf ops wt prune

Prune worktrees, completed build output, or finished branches

| Argument | What it does |
|---|---|
| `--dry-run` | Show what would be pruned without removing anything Default: false. |
| `--builds` | Reclaim old generated output from completed Tasks; retain checkouts Default: false. |
| `--branches` | Delete origin and local branches tied to merged or closed PRs, finished Tasks, or published release candidates Default: false. |
| `--json` | Emit cleanup entries and allocated-byte receipts Default: false. |
| `--retention-hours` | Keep output modified within this many hours Default: 24. |
| `--maximum-roots` | Maximum generated roots removed per invocation Default: 8. |
| `--maximum-bytes` | Maximum allocated bytes removed per invocation Default: 68719476736. |
| `--help / -h` | Print help |

## lf ops wt delete

Delete a worktree and its local and remote branch; retain PR and Task outcomes

| Argument | What it does |
|---|---|
| `<name>` | Worktree name to remove |
| `--force / -f` | force Default: false. |
| `--help / -h` | Print help |

## lf ops machine

Name, connect, pool and rent Machines

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf ops machine connect

Install a separate login on an added machine using this laptop's browser

| Argument | What it does |
|---|---|
| `<target>` | target |
| `<provider>` | github, claude, codex, opencode or linear |
| `<email>` | email |
| `--chrome-profile` | chrome profile |
| `--help / -h` | Print help |

## lf ops machine credentials

Inspect or receive a machine credential (credential bytes use stdin only)

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf ops machine credentials inspect

Report whether this account is installed, without logging in

| Argument | What it does |
|---|---|
| `<provider>` | provider |
| `<login>` | login |
| `--help / -h` | Print help |

## lf ops machine credentials receive

Receive a fresh login as JSON on stdin; preserve existing accounts

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf ops machine public

Name this Machine for teammates, or `off` to make it private again. A private Machine's ownership entries carry only its id

| Argument | What it does |
|---|---|
| `<name>` | name |
| `--help / -h` | Print help |

## lf ops machine id

Print this machine's stable local Machine identity

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf ops machine add

Discover a remote machine and save its SSH destination

| Argument | What it does |
|---|---|
| `<target>` | target |
| `--label` | label |
| `--pool` | The pool of machines you own it joins |
| `--cost-usd` | What it cost you, committed when added |
| `--expires-at` | After this RFC 3339 time it takes no new work |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf ops machine list

List saved machines without connecting

| Argument | What it does |
|---|---|
| `<label>` | label |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf ops machine status

Check reachability and version without prompting

| Argument | What it does |
|---|---|
| `<label>` | label |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf ops machine use

Run work on a Machine: set a Wave's or this repository's default. Bare, it applies to the worktree's Wave, else this repository; `lf task move --machine` moves one Task

| Argument | What it does |
|---|---|
| `<machine>` | A registered Machine label, this Machine's hostname, or a Machine id |
| `<wave>` | Set this Wave's default for new Tasks and its Wave Session |
| `--session` | Set only where the scope's primary Session runs, and move it there Default: false. |
| `--tasks` | Set only where the scope's new Tasks run Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf ops machine rename

Change a saved machine's label

| Argument | What it does |
|---|---|
| `<label>` | label |
| `<name>` | name |
| `--help / -h` | Print help |

## lf ops machine remove

Forget a connection without touching remote work

| Argument | What it does |
|---|---|
| `<label>` | label |
| `--help / -h` | Print help |

## lf ops machine end

Give a rented machine back now: copy its records, then delete it

| Argument | What it does |
|---|---|
| `<label>` | label |
| `--force` | Delete even though it holds unpushed work, which is lost Default: false. |
| `--help / -h` | Print help |

## lf ops machine pool

Group machines you own; a pool takes new work only on a free machine

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf ops machine pool add

Name a pool of machines you own

| Argument | What it does |
|---|---|
| `<name>` | name |
| `--help / -h` | Print help |

## lf ops machine pool list

Each pool's machines, cost, expiry and what they hold

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf ops machine pool off

Take no new work; what the pool holds stays

| Argument | What it does |
|---|---|
| `<name>` | name |
| `--help / -h` | Print help |

## lf ops machine pool on

Take new work again

| Argument | What it does |
|---|---|
| `<name>` | name |
| `--help / -h` | Print help |

## lf ops machine pool remove

Forget a pool; its machines stay added

| Argument | What it does |
|---|---|
| `<name>` | name |
| `--help / -h` | Print help |

## lf ops machine ondemand

Rent machines from a provider within a limit and an hourly price

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf ops machine ondemand add

Name what to rent and the most it may cost

| Argument | What it does |
|---|---|
| `<name>` | name |
| `--provider` | provider |
| `--vcpus` | vcpus |
| `--memory-gb` | memory gb |
| `--disk-gb` | Modal sizes disk itself Default: 10. |
| `--gpu` | One GPU, on runpod (A40, A6000, L4, RTX4090, L40S, A100, H100), aws (L4, A10G) or modal (T4, L4, A10G, L40S, A100, H100) |
| `--spot` | Interruptible capacity on aws or runpod, bid at the admitted price; the provider may end it early, losing unpushed work Default: false. |
| `--hours` | Hard deadline for each machine it rents, at most 24 Default: 4. |
| `--max` | Machines rented at once |
| `--max-usd-per-hour` | Above this hourly price nothing is rented |
| `--help / -h` | Print help |

## lf ops machine ondemand list

Templates, budgets and the machines each has rented

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf ops machine ondemand max

Change how many machines it rents at once; current machines stay

| Argument | What it does |
|---|---|
| `<name>` | name |
| `<max>` | max |
| `--help / -h` | Print help |

## lf ops machine ondemand rent

Rent a machine by hand; it lasts until its hours, its cap or `lf machine end`

| Argument | What it does |
|---|---|
| `<name>` | name |
| `--hours` | hours |
| `--max-usd` | Spend at most this much on it |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf ops machine ondemand remove

Forget it; its rentals' history stays

| Argument | What it does |
|---|---|
| `<name>` | name |
| `--help / -h` | Print help |

## lf ops people

List the people who own this repository's Tasks, with their GitHub identity

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf ops tokens

Measure this codebase: lines and tokens per directory (tracked files only)

| Argument | What it does |
|---|---|
| `--json` | Emit as JSON Default: false. |
| `--days` | Walk git history instead: the codebase's size on each day it changed |
| `--help / -h` | Print help |

## lf ops account

Refresh account access and capacity, or manage logins and routing

| Argument | What it does |
|---|---|
| `<provider>` | Limit observations to one provider |
| `--cached` | Inspect cached evidence without contacting providers Default: false. |
| `--details` | Include credential sources, browser choices, and timestamps Default: false. |
| `--json` | Emit the account overview as one JSON document Default: false. |
| `--help / -h` | Print help |

## lf ops account disconnect

Disconnect local credentials or one managed login

| Argument | What it does |
|---|---|
| `<provider>` | provider |
| `<email>` | email |
| `--help / -h` | Print help |

## lf ops account connect

Connect local credentials or a managed login using a remembered browser

| Argument | What it does |
|---|---|
| `<provider>` | provider |
| `<email>` | email |
| `--chrome-profile` | chrome profile |
| `--import` | Adopt an existing Claude login Default: false. |
| `--api-key` | Read the provider's API key environment variable Default: false. |
| `--help / -h` | Print help |

## lf ops account set

Edit account configuration or remembered browser choices

| Argument | What it does |
|---|---|
| `<provider>` | provider |
| `<email>` | email |
| `--login-email` | login email |
| `--routing` | routing |
| `--plan` | plan |
| `--clear-plan` | clear plan Default: false. |
| `--paid-through` | paid through |
| `--clear-paid-through` | clear paid through Default: false. |
| `--clear-cooldown` | clear cooldown Default: false. |
| `--chrome-profile` | Replace the ordered browser choices (repeat for fallback profiles) |
| `--clear-chrome-profiles` | clear chrome profiles Default: false. |
| `--help / -h` | Print help |

## lf ops account token

Store a long-lived Claude token for rented machines: mint it with `claude setup-token` in the account's Chrome profile, or read one minted by hand from --file (`-` for stdin)

| Argument | What it does |
|---|---|
| `<provider>` | provider |
| `<email>` | email |
| `--file` | file |
| `--chrome-profile` | chrome profile |
| `--help / -h` | Print help |

## lf ops account redeem-reset

Spend one banked Codex reset for this named login

| Argument | What it does |
|---|---|
| `<provider>` | provider |
| `<email>` | email |
| `--idempotency-key` | Reuse this key when retrying the same redemption |
| `--credit-id` | Opaque credit ID returned by live status (otherwise the service chooses) |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf ops account use

Sign the provider's ordinary home in as a stored login: `lf account <provider> use <email>`

| Argument | What it does |
|---|---|
| `<email>` | email |
| `--help / -h` | Print help |

## lf ops account route

Explain configured and automatic account selection, or replace a route

| Argument | What it does |
|---|---|
| `--repo` | repo |
| `--default` | default Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf ops account route set

Replace a provider's ordered route

| Argument | What it does |
|---|---|
| `<provider>` | provider |
| `<accounts>` | accounts |
| `--repo` | repo |
| `--default` | default Default: false. |
| `--help / -h` | Print help |

## lf config

Read Loopflow configuration

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf config user

Print the configured participant display name

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf config repository

Establish or print this repository's selected planning identity

| Argument | What it does |
|---|---|
| `--bind` | Associate this checkout with a repository identity, retaining prior IDs |
| `--relocate` | Reconnect retained Work after a move predating its Git locator |
| `--path` | Print the selected repository's current local directory Default: false. |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf config planning

Inspect planning synchronization and personal identity

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf config planning key

Show, create, or recover the planning user key that names a `personal: true` ref

| Argument | What it does |
|---|---|
| `--new` | new Default: false. |
| `--recover` | recover |
| `--help / -h` | Print help |

## lf config planning status

Show selection, retained imports and held records without contacting Git

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf self

Manage the installed Loopflow release and exported skills

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf self doctor

Diagnose installation, storage and Process integrity

| Argument | What it does |
|---|---|
| `--planning` | Diagnose repository planning without changing it Default: false. |
| `--json` | Emit the audit as JSON Default: false. |
| `--help / -h` | Print help |

## lf self install

Install the latest published Loopflow release from any directory

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf self install schedule

Install the latest Loopflow at login and weekly by default (macOS launchd)

| Argument | What it does |
|---|---|
| `<frequency>` | Weekly: Monday 09:00; daily: 09:00; otherwise on clock boundaries (local time) Default: weekly. |
| `--help / -h` | Print help |

## lf self sync-skills

Export skills and Graphs into native vendor Skills directories

| Argument | What it does |
|---|---|
| `--yes / -y` | Confirm writes under ~/ without prompting Default: false. |
| `--no-prune` | Keep stale loopflow-generated skills Default: false. |
| `--repo` | Export repository definitions into this checkout instead of home Default: false. |
| `--help / -h` | Print help |

## lf list

Discover commands, skills, and graphs

| Argument | What it does |
|---|---|
| `<path>` | path |
| `--json` | json Default: false. |
| `--help / -h` | Print help |

## lf help

Explain a command, skill, or graph without launching it

| Argument | What it does |
|---|---|
| `<path>` | path |
| `--all` | all Default: false. |
| `--json` | Describe installed command arguments and available result schemas Default: false. |
| `--help / -h` | Print help |

## lf context

Show memory and scratch size targets, their sources, and current usage

| Argument | What it does |
|---|---|
| `--json` | json Default: false. |
| `--wave` | Inspect a Wave's local authored context |
| `--task` | Inspect a Task's checkout and Wave context |
| `--help / -h` | Print help |

## lf clone

Create and use an offline, isolated development data clone (macOS)

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf clone create

Snapshot a source Machine without copying credentials or live authority

| Argument | What it does |
|---|---|
| `<directory>` | directory |
| `--source` | source |
| `--codex-home` | Explicit Codex history home; credentials and configuration are excluded |
| `--claude-home` | Explicit Claude history home; credentials and configuration are excluded |
| `--help / -h` | Print help |

## lf clone run

Run lf (or a Desktop executable after --) inside the offline clone

| Argument | What it does |
|---|---|
| `<directory>` | directory |
| `<command>` | command |
| `--help / -h` | Print help |

## lf clone show

Show the snapshot inventory and continuation limitations

| Argument | What it does |
|---|---|
| `<directory>` | directory |
| `--help / -h` | Print help |

## lf clone dispose

Remove only this owned disposable directory (close its clients first)

| Argument | What it does |
|---|---|
| `<directory>` | directory |
| `--help / -h` | Print help |

## lf portfolio

Select repositories for the Machine-local portfolio

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf portfolio folder

Discover immediate main checkouts in a folder (defaults to the saved folder)

| Argument | What it does |
|---|---|
| `<path>` | path |
| `--help / -h` | Print help |

## lf portfolio manual

Use only explicitly selected repositories

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf portfolio add

Add repositories to the saved manual selection

| Argument | What it does |
|---|---|
| `<paths>` | paths |
| `--help / -h` | Print help |

## lf portfolio remove

Remove manual membership, preserving files and open workspaces

| Argument | What it does |
|---|---|
| `<path>` | path |
| `--help / -h` | Print help |

## lf portfolio import

Import previous Desktop membership only when no selection has been saved

| Argument | What it does |
|---|---|
| `<paths>` | paths |
| `--help / -h` | Print help |

## lf scratch

Manage working notes

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf scratch clear

Remove scratch files added in this branch's diff, keeping existing files and edits

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf discord

Bridge new Discord messages to finite Wave Sessions

| Argument | What it does |
|---|---|
| `--help / -h` | Print help |

## lf discord serve

Poll a configured channel and post each Session's final answer

| Argument | What it does |
|---|---|
| `<wave>` | wave |
| `--help / -h` | Print help |

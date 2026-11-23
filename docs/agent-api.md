# The Agent API

`lf` is both the CLI humans use and the API agents call to launch, steer, and
observe other agents. There is no separate SDK: every command is the same
binary, every read supports `--json` (and `--subscribe --json` for live
streams), and every agent Loopflow launches receives the operating contract
(`LOOPFLOW.md`) automatically.

## Teach an external coding agent to drive `lf`

If you run Claude Code, Codex, OpenCode, Pi, or Antigravity directly outside `lf`, install
the Loopflow skill once:

```bash
npx skills add loopflowstudio/loopflow --skill loopflow -g
```

An external harness acts on your behalf as a Loopflow **User**, the same caller
kind as the Mac app. The skill teaches it to inspect status, start Sessions
you ask for, and work with the Tasks you name.

A Loopflow-launched Wave or Task agent is an internal participant. It receives
`LOOPFLOW.md` automatically and coordinates through Work observations recorded
by Loopflow.

## Delegate work as Tasks

```bash
lf task create --wave <wave> --title "add passkeys"   # file a Task
pbpaste | lf task create --wave <wave>                # title on line 1, notes below
lf task run INF-123                                   # place worktree, start Graph, return once running
lf task run INF-124 --stack-on INF-123                # dependent Task on INF-123's branch
lf sync INF-123                                       # stack the current Task on INF-123
lf task report INF-123 "took the smaller approach"    # post progress (not treated as a steer)
lf task status INF-123 --json                         # read its recorded state
lf task wait INF-123 --until terminal                 # block until done or abandoned
```

**Delegation rule:** only delegate a strict subset that can finish
independently as its own PR. Never delegate the entire problem or the single
blocker in front of you; resolve that inline.

**Children:** pass `--parent <ID>` on `lf task create` (or `lf task update <ID>
--parent <parent|none>`) to attach or detach a child Task; plain `lf task
create` always files top-level work. The parent's interactive conversations
conduct its direct children: they hear when a child stops at an interactive
step, a run of it fails or stops, its PR merges, or it is completed or
abandoned. Acting on that notice is in scope. Steers saved for a child are not
delivered to its parent.

**Stacking rule:** always use `lf task run --stack-on <ID>` or `lf sync <ID>`
(and `lf sync main` to detach). Never `git merge` another Task's branch or run
`gh pr edit --base` directly, because `lf` tracks the stack relationship for
squash-merge survival.

## Run a headless child agent

For focused work inside the current checkout (no separate PR needed):

```bash
lf list                                               # see available Skills and Graphs
lf -b -a codex research 'how does src/sync.rs retry? report; edit nothing'
```

`lf -b` runs headless in the current checkout, prints the result, and exits.
Unlike a harness's built-in subagent, it can run a different harness or model,
records a Session, and attributes cost to the current Task. Redirect a running
child with a steer:

```bash
lf session list --headless                            # find the child's Session ID
lf steer SESSION 'skip the vendored code'             # read after its next tool call
```

## Steer and interrupt

```bash
lf steer INF-123 "keep the public API"                # every Session on the Task (syncs to tracker)
lf steer SESSION "skip the vendored code"             # one Session
lf --wave parser steer "tokenizer first"              # every Session on the Wave
lf --repo . steer "hold merges"                       # every Session in the repository
lf session steers SESSION --json                      # pending vs. delivered
lf task interrupt INF-123                             # stop its running Graph
```

Running Sessions read steers after their next tool call; idle work sessions
wake automatically. A steer naming a stopped interactive Session starts it in
a held terminal. A Task steer also starts its stopped primary conversation.
Use `lf -b steer SESSION TEXT` to run a stopped conversation’s turn here. Use `lf task report` for progress notes so they are not
delivered back as steers.

A stopped or crashed Graph stays as history and never resumes. Read
`lf task status INF-123 --json` and `lf graph show <RUN_ID> --json`, then
continue remaining steps with `lf task run INF-123`.

## Ship

Pick the commitment level that matches the state of the work:

```bash
lf pr publish    # ready PR, headless; the agent default
lf pr submit     # synced, scratch cleared, ready for a person's merge click
lf land          # request auto-merge (add --wait-and-fix to repair CI until merged)
```

Reserve `lf pr open` (which opens a browser) for when someone explicitly asked
to view the PR.

## Observe (`--json` and `--subscribe`)

Every read surface returns structured JSON, and works remotely with
`lf --machine <id>`:

```bash
lf wave list --json                                   # Waves and runtime status
lf wave status <wave> --json                          # its Project, Tasks and execution evidence
lf wave show --json                                   # every Wave's plan joined to runtime state
lf task list --json                                   # open Tasks (same records Desktop renders)
lf task status INF-123 --json                         # one Task, its Sessions, Graph runs, and thread
lf session list --json --select id,title,state        # select specific fields only
lf session history --task INF-123 --json              # chronological turns and outcomes
lf session history show SESSION --instructions --json # why each instruction/skill was included
lf session usage --task INF-123 --json                # token and cost breakdown
lf session usage --weekly --json                      # weekly context cost and turn latency
lf graph list --runs --state current --json           # OS-confirmed live Graph runs
```

Add `--subscribe` to `lf task list`, `lf task status`, `lf wave show`,
`lf wave status`, `lf session list`, `lf session history`, or `lf repo tracker`
for live NDJSON updates until stdin closes.

Use `lf server status` for local client server health, `lf desktop` for Desktop
pane automation, and `lf --machine <id>` to run any command on a remote
Machine. See [Telemetry](telemetry.md) and [Surfaces](surfaces.md) for full
schemas.

## The owners

Tracked Work follows Wave → Task in navigation. A Wave owns its objective and
memory. Its Project groups all its Tasks.
A Task owns its checkout and its PR history, with one current PR. Every
conversation and Graph attributed to it is part of its work. The Task has one
captured graph and position, advanced by `lf task run` and `lf task auto on`; independent
Graph runs do not move that position.

LfSession keeps a continuable conversation, whether interactive or headless,
and records the lf attached to it and the agent it runs: pid, OS start time and
exit. A Graph run is one lf process and the steps it starts; the run and each
step record their own pid, start, end, exit code and error. No table records lf
commands themselves: one an agent issues is a tool call in its transcript.
Alive means the OS still has the recorded pid with the recorded start time.
Provider turns and retries remain history inside the conversation; they do not
create another generic execution object.

A recorded pid grants neither process-control authority nor permission to move
a Graph cursor. Each attachment starts a new agent with a fresh token, which
fences out the old writer; the conversation keeps its identity.
Passive observation acquires no claim. See the [contract and cutover status](dev/reference-cutover.md#cutover-status)
for the remaining reader, wire and lifecycle conversion.

## The contract

Loopflow launches receive a small launcher contract: work in the supplied
checkout, follow the requested tools and repository conventions, and answer in
the current Session. Standalone native skills retain their own guidance.

The Task model and its commands are in that text, so every launch carries
them. Association supplies context and ownership; the selected skill or Graph supplies
the procedure. Keep repeatable instructions with that skill and repository rules
in its agent guide.

## Memory and cleanup

- **Wave memory:** `.lf/wave/<name>/MEMORY.md` is plain Markdown; edit it
  directly as part of Graph work rather than through a special API.
- **Deleting Tasks:** `lf task delete ISSUE` cancels unfinished placed work or
  cleans completed delivery before removing the tracker issue (safe to retry;
  blocked by live runs or dirty checkouts).
- **Ownership safety:** automated commit, PR, and completion commands fail
  closed if Linear moved an issue to another Project, leaving the checkout
  intact for inspection.

Next: [Configuration](config.md) · [`lf` reference](lf-reference.md).

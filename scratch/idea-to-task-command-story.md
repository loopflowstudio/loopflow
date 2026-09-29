# One story: Dave takes an idea to running work

2026-09-29. Command-selection storyboard requested by Jack Heart for
[LOO-334's design](resolve-tasks-from-linear-and.md). Dave is a fictional composite:
a technically capable founder/CTO who sometimes types commands and usually lets
an agent handle the details. This is an intended interaction, not an observed demo.
It assumes an existing repository and a usable agent login. First installation is
not the opening scene.

## 1. A customer asks for something

A customer wants to download their invoice history. Dave has twenty minutes to
shape the feature before a meeting. In the repository, he starts:

```sh
lf skill design
```

Dave: “Customers need a CSV of their invoices. Help me work out the smallest
version worth shipping.”

Loopflow creates a worktree for the design and opens the conversation there.
The agent asks about the product behavior, inspects the application, and writes
one design into that worktree's scratch directory. Dave settles date filtering
and the exported columns. No Task, Linear connection or chapter is required to
explore the idea.

The worktree is concrete but its management is incidental. The session shows
where it is working and gives the next command in that context. It cannot assume
that creating a worktree changes the parent shell's current directory.

For someone who wants to name the workspace first, the equivalent explicit start
is `lf wt create invoice-export`, then `lf skill design` inside that worktree.
The design skill reuses a suitable current worktree rather than creating another.
The automatic start is the main story; these are two entrances to the same work.

## 2. Dave decides to build it

In the design worktree:

```sh
lf skill launch-plan
```

Dave: “Build this. Keep it to one Task.”

The agent reads the design already present, resolves its planning placement and
checks whether a Task already owns this outcome. It creates one only if needed,
then supplies the actual design to the Task's execution worktree before launching.
If the destination differs, the handoff names it and verifies the design arrived.
The agent must not restart the design interview or launch two writers.

The reply is short: “APP-42 — Export invoice history. Started. Design: <path>.”
APP-42 is an illustrative returned identifier, not a fixed identifier to type.
A local-only repository returns a local Task ID and follows the same interaction.
If planning placement genuinely needs Dave's choice, ask for that choice here;
do not ask him to repair an empty cache or manufacture a chapter receipt.

The connection is a repository setting already configured when present. Linear
receives connected planning writes; otherwise they go to the local store. Dave
does not choose a different family of Task commands for those cases.

## 3. Work now has a handle

Before the meeting, Dave checks the work:

```sh
lf task status APP-42
```

It identifies the feature, reports starting/running/review-needed as appropriate,
and gives the next useful action. A different selected data directory still finds
the planning and locates the existing execution. Dave is not instructed to inspect
SQLite or set Home environment variables just to find his Task.

Dave remembers one constraint:

```sh
lf task comment APP-42 "Use the current date filter for the export."
```

The comment becomes Task direction; it does not start another worker. Dave can
also say “Tell the export Task to use the current date filter” to his agent, which
uses the same command. The agent does not need a private orchestration API.

If work has stopped at a resumable boundary and Dave later asks it to continue:

```sh
lf task run APP-42
```

This continues the saved Task Flow. It does not create a replacement Task or reset
its progress. The configured Flow's review and delivery boundaries still apply.

## 4. An account matters only when work needs one

Later the Task reports that its available agent account cannot currently run.
Now Dave has a reason to inspect accounts:

```sh
lf auth status
```

If another login is needed, the agent walks him through the existing connection:

```sh
lf auth connect codex dave@example.com
lf task run APP-42
```

The address is illustrative. Account selection remains the existing explicit
account controls; this story does not imply that adding a login always changes
routing or clears a provider limit. The response must identify any remaining
account problem. Current command spelling is `auth`; a proposed `account` rename
is not silently introduced by this story.

## 5. Dave checks the work, then gets back to his day

```sh
lf runs --active --task APP-42
lf usage --task APP-42
```

These answer “Is it doing something?” and “What has this work used?” Only now do
Runs and usage appear. Unknown measurements stay unknown. If Dave wants a live
machine-wide process view, `lf top` is the existing monitor surface. There is no
new `monitor` command implied here.

Dave's agent can read the same results with `--json` and summarize them. Routine
work still uses the official lf automatically; an installation update is reflected
at the next worker boundary. Runtime pins and data-store paths belong in details
when investigating a problem, not in the feature's opening instructions.

## 6. A shipped feature reveals two continuing needs

Dave reviews the export and the Task reaches delivery through its chosen Flow.
Customers use it. Their next requests go in two directions: accountants need to
trust refunds and tax totals, and customers want more things they can do without
asking support.

Dave: “Keep improving both of those. Set up future work around billing accuracy
and customer self-service.”

The agent checks `lf wave list` for existing owners before proposing anything
new. In this story neither responsibility has a Wave yet. The proposed creation
surface is:

```sh
lf wave create billing --objective "Customers and accountants can trust every invoice total."
lf wave create self-service --objective "Customers can get routine account information without contacting support."
```

These are proposed command spellings: current source has no `wave create` command.
They express the desired interaction for review, rather than claiming a working
CLI or resolving all Wave-existence policy. Both Waves inherit the repository's
planning connection; neither asks Dave to connect Linear again.

The agent helps Dave give each Wave a first plan. Billing will prove that exported
refund and tax totals reconcile against the ledger. Self-service will prove that
customers can find and download the information they need without a support
intervention. The objectives endure beyond these first Tasks. The agent handles
the existing chapter-plan operation; Dave does not author internal Project IDs or
JSON just to express these outcomes.

Dave: “Queue the next changes. Don't start them until I've reviewed the briefs.”

Once each Wave has a real current plan, the agent uses the Task commands Dave
already encountered:

```sh
lf task create --wave billing --title "Show refunds correctly in invoice exports"
lf task create --wave self-service --title "Let customers download their monthly invoice summary"
```

The agent supplies the briefs and success criteria through the command's existing
notes/stdin input. These are planning-only creations, with no `--run`.
The original export stays one completed Task in its history; creating two Waves
does not duplicate it or launch another copy of its worker.

Dave ends with:

```sh
lf wave status billing
lf wave status self-service
```

Each shows what is being improved, how progress will be judged and the queued
Task. Dave now has two places to return to with future direction, both rooted in
what the first feature taught him.

## What this story selects

Dave can commit the two Waves' goals, memory, Flows and Skills for a colleague to reuse. Without
Linear, that colleague has an independent private plan; pulling the repository
does not copy Dave's Tasks or running work. Linear adds shared planning when the
company needs it. Shared execution is a paid layer, outside this story's scope.

The command progression is workspace → design → launch-plan → Task → accounts
when needed → monitoring → Waves for future work. Teach commands when the
character needs their outcome. Waves enter after a delivered feature reveals
enduring responsibilities, rather than as onboarding terminology.
Design and launch-plan form a continuous handoff; Task commands become useful
when work has a durable identity. Explicit `wt create` remains an optional entry.
The same story supports direct use and an agent executing documented commands.

Automatic design worktree creation is Jack's newly requested target behavior;
source inspection has not established that it already works. Current `wt create`
requires a name. Current launch-plan can stage a design into a prepared Task
checkout, but does not adopt an arbitrary existing implementation worktree.
Do not conceal either placement gap in a purported current CLI demonstration.
The storyboard's auto-placement/context handoff is an additional design requirement;
trace it through launch before claiming this experience is implemented.

Acceptance should replay this sequence in a disposable repository: automatic
workspace, design artifact, verified launch-plan handoff, one real Task execution,
status/steer/continuation from another store, truthful account/usage reads, delivery,
and planning two future outcomes under Waves. Future Task creation must allocate
no execution. Wave creation/bootstrap remains an explicit product-surface gap;
it must not be treated as implemented because a storyboard uses proposed commands.
Existing focused sync/runtime proofs remain necessary underneath. No commands in this storyboard were executed against live
work or providers while authoring it.

## Companion counterexample: a canceled Task still has a useful checkout

This is a separate acceptance case, not the main story's ending. If APP-42 were
canceled, `lf task run APP-42` would refuse managed progression. Dave could still
run `lf flow code "Prototype a printable invoice summary in this checkout."` in
its worktree. That ordinary Flow cannot revive APP-42 or settle its invocation.
The successful main story and this failure case must both work.

## Possible epilogue: the company adopts Linear

Dave's billing and self-service Waves began locally. Colleagues could already
share their committed goals, memory, Flows and Skills, while keeping private plans. When he connects the repo
to the company's Linear workspace, Loopflow previews their migration: link billing
to an existing Finance Initiative, create a self-service Initiative, and show
the Project/Task mappings before applying anything. Afterward, the same Wave/Task
commands read Linear-backed shared planning. No workers, claims or execution
control are shared by that connection. A colleague pulls the repository's Wave definitions and Initiative bindings,
then syncs their shared planning. Unrelated company Initiatives do not become
Waves automatically, and cloning does not create duplicate Initiatives.
The [Wave-existence proposal](wave-existence-and-linear-migration.md) records this
new review direction and its unresolved migration details.

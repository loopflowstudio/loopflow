# Open questions — work with children, and where plans live

Only what is still open as of 2026-11-18 (kickoff). Words and earlier rounds:
`scratch/said.md`. Work with children is planned in `scratch/plan.md`; the
rest is not chosen.

## Made at kickoff with nobody present, for review

- The checkout is retired at close, as today, not at merge. A merged parent's
  conversation stays in its checkout; after close it reopens in the
  repository's main checkout.
- A child copies its parent's Project when filed; nothing climbs at read time.
- `lf task complete <parent>` never stops and takes no new flag: merged
  children become done, never-started ones are abandoned, and a child with
  anything unmerged stays open with its link and is named (concept review;
  replaces the kickoff's `--keep` / `--abandon`).
- A conversation belongs to one Task and conducts that Task's children: two
  relations. `session_tasks()` is not widened (concept review).
- A conductor is told when a child stops at an interactive step, fails,
  merges or closes, not at each step.
- A closed parent's conversation still hears about a child that is open.
- No `merged` / `closed` booleans: `pr.phase` and `state` already say it.
- Removing several PRs in a row on one Task is a follow-up Task, not this PR.
  It is live behaviour (v0.13.16, `docs/ops.md:38`).
- Wave and repository conversations do not hear about Tasks in this change.

## Made while building slices 1 and 2, for review

- Only a done or abandoned Task's conversation falls back to the main
  checkout. An open Task with no checkout still refuses with "has no
  checkout; place it with `lf checkout`".
- `lf resume <Task>` still chooses among the Task's unfinished conversations
  only; a completed one reopens by Session ID.
- A child's `lf task status` prints `parent: LOO-12` (answers the third
  question below for the detail view; lists are not built).
- A child line's standing is the Task's state plus the step it stands at:
  `moving`, `idle at human-review`, `unstarted`. The docs example changed
  from "running at implement" to match; a running child's current step is
  not on `TaskRow`.
- `--no-wave` and an omitted `--wave` are the same value, so a follow-up
  filed with `--no-wave` still joins its parent's Wave. `--parent none` is
  how to file outside it.
- A follow-up filed in another repository than its parent keeps the link and
  takes no Project.

## Made while building slice 3, for review

- A child's first reading is compared with an empty one, so a child nobody
  read before it stopped still produces a notice. A Task's own conversation
  keeps the baseline rule.
- A conversation that just started marks the notices already written as read,
  like updates: it reads its children with `lf task status`.
- A stored reading this build cannot parse is treated as no reading (a
  baseline) instead of an error. Readings saved before this change lack `pr`
  and `closed`, so each Task's next reading reports nothing.
- A closed child that reopens sends "was reopened".
- The notice header a conversation sees: "Task notice <id>. News about work
  this conversation conducts; following it up is yours. Read the Task's
  current status before acting."
- A run that ends without succeeding and leaves the Task at an interactive
  step sends both in one notice: "failed at gate and stopped at human-review".

## Made while building slice 4, for review

- A parent with open children whose run reaches `end` is not approved to
  ship and requests no completion. If its PR is still open there, it waits
  for `lf land`.
- `lf task complete <parent>` prints children in the order done, abandoned,
  left open, as the docs example has it, not filing order.
- "Never started": unstarted, not begun, no PR row, no Graph process and no
  checkout event. A child with a checkout and no commits stays open.
- A child abandoned by its parent's completion records the reason "parent
  Task completed".
- In `lf wave show`, a child prints under its parent in the parent's
  section, with its own section named at the end of the line when it
  differs.

## Made while finishing slice 2, for review

- The creation receipt's `parent` is the parent's whole `TaskRow`, the row
  lists print, so `.parent.identifier` holds and no new shape exists.
  `task.parent_task_id` (the stored Task) and `row.task.parent_id` both
  hold the ID.
- Served `task.create` has no caller, so `parent: null` files top-level;
  a client names the parent or gets none.
- `tests/fixtures/dto/task_status.json` is now read as `DesktopTaskPane`,
  what `lf task show --json` prints, with `row: null`.

## Made while building slice 5, for review

- A tracker's or another Machine's parent that would make a Task its own
  ancestor, or that names a Task this Machine does not have, is skipped
  without an error: the Task keeps its parent. Only a local save errors,
  and no command makes one yet. Skip quietly / record a Task event / show
  it as a conflicted tracker action?
- A swap made in the tracker (A under B becomes B under A) read child
  first is skipped as a cycle, and is not retried until that issue is read
  again.
- A tracker item with no parent clears the Task's parent only when the held
  parent has a tracker item. A parent in another repository or one never
  sent stays.
- A parent in another repository does not travel with a repository's plan
  page; the other Machine has the child without the link.
- An existing Linear sub-issue becomes a child here at its next read.
  Wanted for every sub-issue already in Linear / only for Tasks filed from
  Loopflow?
- Asana: a follow-up is created as a subtask (one extra `GET` for the
  parent's gid); a subtask made or moved in Asana is not read back. Store
  Asana gids so it can be / leave?
- `docs/work.md` says a coworker's sub-issue becomes a child for Linear
  only, since Asana's is not read. Restore "or subtask" when it is.
- The Linear stand-ins accept any input key, so a mutation carrying a field
  Linear does not have passes every test (a stray `"parent":null` in the
  reorder input did). Have the stand-in refuse keys outside each input type
  / leave to the live check?
- `TrackerItem.parent_observed` takes `#[serde(default)]` so captured
  inventories in test fixtures still decode. The type is not an `lf --json`
  DTO.

## Made at the concept review of the built branch, for review (slice 6)

- Flat only to an open parent: filed from a child whose parent is done,
  abandoned or not on this Machine, a follow-up goes under the calling Task.
  Passed over: under the closed parent as built; top-level.
- A child completed by its parent's completion settles its own children by
  the same rule.
- A relaunch that fails at the step the last run failed at is not a second
  notice. A person's deliberate rerun failing the same way is also silent to
  the conductor until the Task's step or PR moves.
- `parent_id` is the ID and `parent` is the parent's `TaskRow` everywhere;
  `lf task status --json`'s `parent` changes from an identifier string to a
  row.
- `lf task complete --json` and served `task.complete` gain `children`.
- Left as built: one notice per child settled by a parent's completion, sent
  to the conversation that completed it; a terminal in the Task's checkout
  files top-level.

Made while building slice 6:

- The repeated failure stays silent only while the Task's step and PR phase
  match the earlier reading; either moving makes the same sentence news
  again. Keep that reset / silence it for the life of the Task?
- `lf task complete --json` keeps the Task's own fields at the top level
  (the stored Task, as before) and adds `children`, whose entries hold a
  `TaskRow` under `task`. The parent and its children are two shapes in one
  answer. Leave / make the top level a `TaskRow` too (breaks Desktop's
  decode of `task.complete`)?
- A child left open has `children: []` in the answer even when it has
  children: the list says what the completion did, not what exists.

## Raised by QA on a private Machine (2026-11-18, `scratch/qa.html`)

Changed there, for review: the human `lf task create` receipt prints a second
line `  parent: <ID>` for a child; a child line in `lf task status` ends with
its PR (`active at demo · #100 merged`); `lf task complete` says `stays open:
started, no PR` where it gave no reason; a run and its relaunches failing at
one step in one reading are one "failed at …" in the notice.

Seen there and changed by slice 6 (choices in the section above): a
follow-up filed from a child's conversation after the parent closed went
under the closed parent; `lf task complete <parent>` left a merged child's
never-started child open (`bd062e` under `c2b6ef`); `lf task run`'s two
relaunches of one broken child woke its conductor twice with the same
sentence. Still as QA saw it: the closed parent's own reopened conversation
files under the closed parent. Under it / top-level?

- A person typing `lf task create` in a terminal inside the Task's checkout
  files top-level work: only the conversation carries the caller. Should the
  checkout count?
- A real Claude conductor, told a child stopped at review, read its status and
  asked "should I continue it with `lf task run`?" Is that the wanted
  behaviour, or should `task-session` say more about deciding a child's
  review?

## Raised by the second QA pass, after slice 6 (2026-11-18, `scratch/qa.html`)

Nothing in the code changed in this pass. Seen and left as built:

- `lf task complete <parent>` abandons a never-started child and leaves that
  child's own never-started child open, top-level in the backlog (`e1dd86`
  under `a1e338`). `lf task abandon <ID>` typed directly leaves children
  alone the same way. Abandon them too / leave?
- A parent with no PR whose Graph reaches `end` with a child open stays open
  and reads `action: run (run the next work with lf task run …)`. Only the
  merged case says "N children open". Say it here too / leave?
- A child whose last run failed prints `child: 7e85ee  Log each retry  idle`
  on its parent's status; the failure is in the child's own `reason` only.
- A second Machine lists Tasks abandoned on the first as open backlog, the
  children a parent's completion abandoned among them. An arriving
  `canceled` disposition sets no `abandoned_at`; `main` reads the same way.
- Served `task.create` files top-level when the `parent` key is absent, as
  `title`, `wave`, `graph` and `session` are accepted absent. Its unknown
  parent answers `outcome_unknown` with the CLI's words, "--parent names an
  unknown Task: nope99; inspect the affected work before retrying", though
  nothing was written.
- The earlier pass's question stands and now has an answer to check: told a
  child failed, real Claude read the child, said it had not relaunched it and
  stopped; told a child stopped at `demo`, it named the conversation to open.

## Made at the third concept review, for review (slice 7)

Answers two of the second QA pass's items above; the rest stay as listed.

- A no-PR parent at `end` with children open reads "Finished · N children
  open", then "Finished; complete the Task". Passed over: "run the next
  work" as built; completing it at `end`.
- A parent's completion settles the children of a child it abandons, as it
  does for one it finishes. `lf task abandon <parent>` typed directly still
  touches one Task. Should abandoning a parent settle its children too?
- Not taken on, on the page as larger than this work: a second Machine
  reading Tasks abandoned elsewhere as open (`planning_state='canceled'`
  with no `abandoned_at`; `task_open_sql` does not read it, so a parent
  there counts them in "N children open"). Its own Task, with the plan
  exchange?

## Raised by reading slice 4 (not run)

- `lf task complete <parent>` closes or abandons children, and each of
  those is a standing move its conductor is told about (decision 6). The
  conversation that just completed the parent then gets one notice per
  child it settled. Send them as now / skip notices for children settled by
  their parent's completion?
  Seen in QA: the completing conversation's next prompt carried its own
  "Task completed" update and one notice per settled child.

## Raised by writing the docs (2026-11-18)

- "Parent" already means the stack parent across `docs/work.md`,
  `docs/agent-api.md` and `docs/ops.md` ("forks from its parent's branch").
  The docs now say "a parent says where a Task came from, a stack says which
  branch it starts from". Is the filing link called parent / follow-up of /
  something else, and does the stack keep the word?
- A follow-up that needs its parent's unmerged code must also pass
  `--stack-on`. Filed from a Task whose PR has not merged, should a child
  stack on it by default? The docs say no (plan: neither implies the other).
- A child whose parent closed is listed top-level. Does anything on that
  child still show where it came from in the human output, or only in
  `--json`?

## One type of work

1. (planned: see above and plan decision 7)
2. (planned: a sibling; `--parent` says otherwise)
3. The fusion of Wave and Task: behaviour first with storage later / one type
   in the store from the start?
4. Is "work" the name for everything, with Task and Wave kept as words for
   what a piece of work is doing?
5. Children in the sidebar: folded behind a count until one needs you / always
   listed / listed while the parent's conversation is open?
6. (answered: both providers resume from another directory after the first
   is deleted; probes in `scratch/said.md`)
7. How one tree maps onto a tracker with Projects, issues and sub-issues, and
   onto LOO-938's Wave and Project records.

8a. A folder (a short-lived group): a tracker Project of its own / a group
    only its owner sees, its Tasks showing in the parent's Project / a Task
    with subtasks? Is it a third kind, or work with children that has no PR,
    plus a choice of how the team sees it?
8b. May a parent have no PR and no checkout of its own, and where does its
    conversation run then?

## Plans and docs

8. Who writes the shared plan most of the time: the person / the agent with
   review / both in one doc?
9. A doc store as a repository setting like the tracker / docs as links of any
   kind on a piece of work?
10. With no outside doc tool, the plan file lives: in the repository with a PR
    per change / with the planning records outside the working tree?
11. A follow-up gets the plan as it was reviewed / the latest?
12. Does a running Task see amendments made after it started?
13. Do comments and edits people make in the doc reach the conversation that
    owns the work?

## Dependencies (its own design)

14. One link, "follow-up of", with waiting and starting branch as settings on
    it / separate relations?
15. The Task waited on is abandoned or redone: the parent's conversation is
    woken and asks / waiting Tasks start from main / they stay waiting and say
    why?
16. May a Task wait on several?

## Shipping

17. `lf ship`, `-b` and the Auto-Ship approval: which one stays, and what does
    `ship` become shorthand for? Not read in the code yet.
18. A follow-up runs to merge because of how it was filed / its parent's
    conversation answers its stops?
19. What shows that something merged without being opened?

## Doc edits reaching children (its own Task)

20. What closes a batch of doc edits: a quiet spell / someone saying "send
    this" / nothing, each child reads the latest at its next step?
21. Does every child get the batch, or does the parent's conversation read it
    and steer the children it touches?

## Not checked

`scratch/explore.html` round six (the whole window as sequences) has not been
looked at rendered, and predates the one-type direction.

## Slice 7 (made for review)

- "No PR" is no PR record at all, the same test `lf task complete` uses
  (`ops::task::completion_refusal`). A Task at `end` with an unpublished PR
  record (phase `working`) still reads "run the next work with `lf task
  run`", because completing it is refused. Should that one read "Finished"
  too, and complete?
- A finished Task nobody owns still reads "Unassigned; nothing moves it until
  `lf task claim` or `lf task run`" once its children are closed, while
  `next` is `lf task complete <ID>`. Merged Tasks did this before this work.
  With children open the "Finished · N children open" line wins. Should
  finished outrank unassigned?
- A never-started child with a started child of its own: the first is
  abandoned, the second stays open and is listed top-level by the flat rule.

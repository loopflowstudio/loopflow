# LOO-1094: explicit children on parent completion, reopen abandoned Tasks

## Outcome

Completing a parent never abandons work by surprise. `lf task complete LOO-12`
on a Task with open children refuses and lists them:

```text
$ lf task complete LOO-12
Error: LOO-12 has 3 open children; say what happens to them:
  LOO-14  Log each retry        #1980 merged
  LOO-15  Retry budget          #1981 open, at ship-review
  LOO-16  Backoff docs          unstarted
  lf task complete LOO-12 --children rehome    # they move up to LOO-10 and stay open
  lf task complete LOO-12 --children abandon   # merged ones done, the rest abandoned
```

(`to LOO-10` names the grandparent; with none, "they become top-level Tasks".)

```text
$ lf task complete LOO-12 --children rehome
LOO-12 done
  LOO-14 moved up to LOO-10
  LOO-15 moved up to LOO-10
$ lf task complete LOO-12 --children abandon
LOO-12 done
  LOO-14 done: #1980 merged
  LOO-15 abandoned: #1981 open, at ship-review
```

`--children abandon` refuses, writing nothing, while any child it would abandon
has live execution, and names each blocker. `lf task reopen LOO-15` works on an
abandoned Task. Desktop and iOS ask the same question in one dialog.

Steer from Jack Heart, word for word: "Bare `lf task complete` should refuse
when open children exist unless an explicit `--children` option (`rehome` or
`abandon`) is passed. Also design the Desktop and iOS Complete Task
button/swipe UX cleanly for parents with open children so picking between
rehoming and abandoning is natural rather than awkward."

So the open question in the brief (refuse vs keep-linked default) is settled:
refuse. A completed parent with open children linked under it is still a
reachable state (Graph-end completion waits; a child reopened under a done
parent), and `waves.rs::task_hierarchy` keeps listing those children on their
own; nothing there changes.

## Source of truth

The completion request event. Today `task_events.kind` is
`completion_requested` (Graph end; waits while a child is open) or
`family_completion_requested` (explicit; settles children by abandoning). The
request can pend until the PR merges and is settled later by
`settle_pending_completion` (`ops/task.rs:1150`, `:3240`), so the child policy
must live on the request, not on the call.

- `TaskEventKind::FamilyCompletionRequested { reason }` keeps its bytes and
  its meaning: abandon. Doc comment says so.
- New `TaskEventKind::RehomingCompletionRequested { reason }`
  (`rehoming_completion_requested`).
- `children::completion_request` returns `Option<ChildPolicy>` instead of
  `family: bool`. `ChildPolicy { Rehome, Abandon }` is one `ValueEnum` +
  serde enum in `loopflow-storage` (`sqlite/children.rs`, beside
  `ChildOutcome`), used by Clap, `Write::TaskComplete` and the request.

Why a new kind instead of a `children` field on the old one: a released `lf`
reading a pending rehome request written as `family_completion_requested`
would settle it by abandoning. Released `lf` compares the kind string in SQL
(`completion_request`), so an unknown kind reads as non-family: it waits for
children and clears the request. Safe skew. Cost: released `lf` cannot parse
that one event in history (unknown variant); same as every new event kind.

## Design

### Storage (`rust/loopflow-storage/src/sqlite/children.rs`)

- `request_family_completion(task, note, policy: ChildPolicy)` writes the kind
  for the policy. A pending request with a different policy is replaced (same
  rule as family-over-plain today).
- `complete_task` matches the policy:
  - `None` (Graph end): unchanged, `WaitingForChildren` while children are open.
  - `Abandon`: `complete_children` as today.
  - `Rehome`: `rehome_children(tx, parent)`: each direct open child gets
    `Edit::TaskParent(parent.parent_task_id)` through
    `planning_write::local`, the same edit `lf task update --parent` makes, so
    tracker actions queue in the same transaction. Closed children keep their
    link as history. Outcome `ChildOutcome::Rehomed`, `children: []` (their own
    children come along untouched).
- `ChildOutcome` gains `Rehomed`; `CompletedChild.task` (a `TaskRow`) already
  shows the new parent, so `lf task complete --json` needs no other field.
- `reopen_task`: the edit already in this checkout (clears `abandoned_at`,
  `abandon_reason`, completion request/error; `canceled`→`started`/`unstarted`;
  clears `task_auto`). Check `task_summaries` does not keep an `Abandoned` end
  row that the board reads after reopen; clear it the way completed reopen does.

### Execution (`rust/loopflow-execution/src/ops/task.rs`)

- `task_complete(repo, issue, note, children: Option<ChildPolicy>)`:
  - `None` and `open_task_child_count > 0` → refuse with the listing above
    (`TaskRow::standing()` per child, the grandparent identifier or "top-level").
    Nothing is written. A leaf completes as today.
  - `Some(policy)` → `request_family_completion(.., policy)`, then
    `settle_completion`.
- Live guard, in `settle_completion` before `store.complete_task`, only when the
  pending request is `Abandon`: walk open descendants; for each whose active PR
  is not merged, `task_execution_blockers(&store.sqlite, id)`. Any blockers →
  error `"LOO-15 is running: <blocker>; stop it or use --children rehome"`; the
  request records it as `completion_error` (existing path), nothing is
  abandoned. A rehome needs no guard: moving a parent link does not touch
  execution.
- After `Settled`, `Rehomed` children get `sync_completion` (tracker parent
  change) and nothing else.
- `task_reopen` unchanged apart from dropping any "abandoned" refusal text.

### CLI and wire (`rust/loopflow/src/lf/`)

- `TaskCommand::Complete { .., children: Option<ChildPolicy> }`, `--children
  <rehome|abandon>`, `after_help` saying what each does and that the flag is
  required when children are open.
- `write.rs`: `Write::TaskComplete { directory, issue, children:
  Option<ChildPolicy> }` with `#[serde(deserialize_with =
  "Option::deserialize")]`; server schema (`server/schema.rs:495`) and
  `tests/fixtures/dto/` (server_schema.json and the write fixture that carries
  `task.complete`) updated in the same change.
- `write_child_completions` prints `moved up to LOO-10` / `moved to top level`.
- `lf task reopen` help: works on done and abandoned Tasks.

### Desktop (`swift/LoopflowMac/`)

- `ServedWrite.taskComplete(directory:issue:children:)`; `TaskChildPolicy`
  enum (`rehome`, `abandon`) in `RegistryQuery.swift`.
- Menu item reads `Complete Task…` when children are open (was
  `Complete Task & Settle Children…`).
- `TaskCompleteConfirmation`: title `Complete LOO-12? 3 children are open`.
  Buttons, in order:
  1. `Complete & Move Children to LOO-10` (or `… to Top Level`), default, not
     destructive.
  2. `Complete & Abandon Children`, `.destructive`, only shown when at least one
     descendant would be abandoned; when every open descendant is merged it
     reads `Complete & Finish Children`, not destructive.
  3. Cancel.
  Message: one line per outcome for the abandon choice, as today, minus
  "cannot be reopened".
- `WorkModel.completeTask(_:children:)` passes the policy; `beginComplete`
  unchanged (direct completion without children sends `nil`).
- Reopen already shows for abandoned Tasks (`isCompleted` = terminal); it now
  succeeds.

### iOS (`swift/LoopflowiOS/TaskListView.swift`)

The list rows (`WorkViewPlan.Content`, Rust `ViewContent` in
`loopflow-storage/src/work/selection.rs`) carry no child facts, so the swipe
cannot know to ask. Add `open_children: u32` to `ViewContent` (required, no
default; Swift `openChildren`), filled where selections resolve
(`loopflow-execution/src/store/selections.rs`), fixtures updated.

- Swipe on a leaf: completes as today.
- Swipe on a parent: the row stays, a `confirmationDialog` asks
  `Complete LOO-12? 3 children are open` with `Move Children Up` and
  `Abandon Children` (destructive) and Cancel; the choice sends
  `.taskComplete(.., children:)`. Same words as Desktop, shorter because the
  phone has no destination name in the row reading.
- A refusal (live child, raced new child) comes back as today's Notice.

### Docs

`docs/work.md` Finish section and the reference paragraph at line ~608: the
refusal, both policies, the live guard, reopen of abandoned Tasks. Delete
"Abandoned Tasks cannot be reopened" in both places.

## What goes

- `family: bool` through `completion_request`/`request_completion`
  (replaced by `Option<ChildPolicy>`).
- The silent-abandon default of bare `lf task complete` and of `task.complete`
  without `children`.
- `reopen_task`'s abandoned refusal (already removed in this checkout).
- Desktop strings `Complete Task & Settle Children…`, `Complete & Settle
  Children`, and "cannot be reopened".
- Tests asserting abandoned Tasks refuse reopen, or that bare complete settles
  children: rewritten to the new behavior, not kept.

Survives: Graph-end completion waiting for children; recursive abandon
semantics; `--json` `children` shape (one more `outcome` value).

## Acceptance check (gate)

Storage/execution fixture, headless: parent P under grandparent G, children A
(merged PR), B (open PR, live Graph), C (unstarted).

1. `task_complete(P, None)` → error naming A, B, C; P open; no event written.
2. `task_complete(P, Some(Abandon))` → error naming B's blocker; A, B, C, P
   unchanged.
3. `task_complete(P, Some(Rehome))` → P done; A, B, C open with
   `parent_task_id = G`; a tracker parent action queued for each.
4. `task_reopen` on an abandoned Task → `started`, `abandoned_at` NULL, auto off.

Plus: `lf task complete --help` shows `--children`; DTO fixture tests (Rust and
Swift) pass for `task.complete` and `ViewContent`; Desktop view test for the
dialog buttons.

## Demo

`lf task create` a parent with two children (one with `lf task run`
detached), then `lf task complete <parent>`: the refusal lists both; `--children
abandon` names the running one; `--children rehome` finishes the parent and
`lf wave show` lists both children top-level. In Desktop, right-click the
parent → `Complete Task…` → the two-choice dialog.

## Passes

1. **Storage + execution + CLI** (could prove the plan wrong: whether the
   rehome edit through `planning_write::local` inside the completion
   transaction queues tracker parent actions cleanly, and whether
   `task_execution_blockers` sees a live child Graph). Policy enum, new event
   kind, refusal, guard, rehome, reopen, CLI flag, wire `children`, fixtures,
   tests 1–4, docs.
2. **Desktop + iOS**: `ServedWrite` field, Desktop dialog, `ViewContent.open_children`
   and the iOS dialog, view tests.

## Exclusions

- No "keep children linked under the done parent" option on completion.
- Rehome moves direct children only, never into another Wave or Project.
- No change to Graph-end (`completion_requested`) behavior.
- Reopen does not restore closed PRs or deleted branches; the next run opens a
  new PR, as reopen of a done Task does today.

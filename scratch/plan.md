# LOO-1101: a bound conversation stays in the worktree it runs in

## The report

> in a worktree i started w/o a task initially (manabot.academy-test-drive), i had an agy lf session going. then i wanted to move to fable, so i did `lf resume -a claude:fable` but it sai there was no work in this worktree

## What happened (read from `~/.lf/loopflow.db`, 2026-11-25)

- `session_94d9963b…` (agy, skill `default`) started at 1795575720 in
  `/Users/jack/src/manabot.academy-test-drive`, a linked worktree no Task owns
  (branch `jack-heart/mtg-388-seat-table`). Its first `processes` row has that cwd.
- At 1795579888 it was bound to MTG-388 (`work_source='bound'`, `bound_at` set,
  row in `task_primary_sessions`). MTG-388 already owned
  `/Users/jack/src/manabot.mtg-388-getting-into-a`, so the bind rewrote
  `agent_sessions.cwd` to that path. The agy process kept running in
  `academy-test-drive`, where the uncommitted work is.
- `lf resume -a claude:fable` from `academy-test-drive`: `checkout_scope` finds no
  owner (`task_checkouts` holds the other path), `checkout_conversation` matches
  on `agent_sessions.cwd` and finds nothing, and resume answers
  "This worktree has no associated work conversation."
  (`rust/loopflow-execution/src/ops/human_session.rs:173`).
- Three fresh `claude:fable` conversations followed in that worktree
  (1795595096, …114, …123), none seeded from the agy thread.
- Second symptom, seen live: the agy process ended at 1795595379 (cause not
  read) and the server's recovery pass restarted it at 1795595387 with
  `--conversation 2c51a976…` in `manabot.mtg-388-getting-into-a`, a checkout
  that has none of the conversation's files. `logs/lf.log`:
  `Task conversation started session=session_94d9963b… attempt=1`.

## Cause

`lf session bind` to a Task that already has a worktree sets the Session's
directory to the Task's worktree. It is the only path that moves a
conversation's directory: `adoption::save(.., continue_in_task = true, ..)`
computes `destination`, and `promote_task_prepared` runs
`UPDATE agent_sessions SET cwd=?2`
(`rust/loopflow-execution/src/ops/task/adoption.rs:191`,
`rust/loopflow-storage/src/sqlite/children.rs:429`). It shipped in #1807
(LOO-582), which recorded the cost as "the accepted limitation: the live
provider continued in its original directory."

Everything else already keeps a bound conversation in place:
`lf task create --session` (`creation_from_a_conversation_keeps_its_directory`),
`admit_scope_workspace` (returns early for `WorkSource::Bound`),
`admit_session_workspace` (`CASE WHEN work_source='bound' THEN cwd`), and a done
Task without a checkout.

## Design: binding changes what a conversation belongs to, never where it runs

Delete the directory move. After `lf session bind`, the Session has its Task and
is the Task's primary; its `cwd` is the directory its provider runs in, as
before the bind.

What follows with no further code:

- `lf resume -a claude:fable` in the conversation's worktree finds it through
  `checkout_conversation` and starts the new thread there, seeded from the agy
  log (`invocation_intent` → `NewThread` → `new_thread`).
- `lf resume MTG-388` / `--task` from anywhere opens the same conversation, in
  the directory it runs in.
- A restart (recovery pass, take over, another harness) launches where the
  files are.
- Session `cwd` and the `processes` launch directory agree again.

Choice made for review. Passed over:

- **Keep the move; have `lf resume` also match a Session's live process
  directory.** A second resolver beside `agent_sessions.cwd`, and it stops
  working at the first restart, which lands the new thread in the Task's
  worktree without the files. That is the state the recovery pass produced here.
- **Move only out of shared checkouts (main, Wave), stay in an unowned linked
  one.** Keeps one more rule than the system has today: creation from a
  conversation on main already leaves it on main.

The cost: a Task whose worktree is X can have its primary conversation in Y.
The Graph's workers still run in X; `lf task status` shows the conversation's
real directory.

## User-visible outcome

In a worktree where a conversation runs, `lf resume` and `lf resume -a <agent>`
open that conversation whether or not it was bound to a Task that has another
worktree. Binding prints the Task it bound to and nothing about directories.

## Demo

```bash
cd ../repo.research && lf -a agy            # a conversation in a Taskless worktree
lf session bind <session> --task LOO-12     # LOO-12 already has ../repo.loo-12
lf resume -a claude:fable                   # same Session, new thread, still in ../repo.research
```

## Acceptance check at gate

`rust/loopflow/tests/session_lifecycle_tests.rs`,
`promotion_sets_directory_immediately_and_resumes_same_native_conversation`,
rewritten and renamed (`binding_keeps_the_conversation_in_its_worktree`):

- after `session bind`, the Session's `cwd`, `task status … sessions[0].cwd`
  and `resume --task … --prepare --json` all report `origin`;
- `resume --prepare --json` run from `origin` with no ID returns the bound
  Session (today: "no associated work conversation");
- the continued launch receives `cwd == origin`; native conversation identity
  and messages are unchanged; `draft.txt` is intact.

```bash
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') cargo test -p loopflow --test session_lifecycle_tests binding_keeps < /dev/null
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') cargo test -p loopflow-execution --lib ops::task::adoption:: < /dev/null
```

## Source of truth

`agent_sessions.cwd` is where the conversation runs. `task_checkouts.worktree`
is where the Task's branch lives. Binding writes `task_id`, `work_source`,
`bound_at` and `task_primary_sessions`; it stops writing `cwd`.

## Affected surfaces

- `lf session bind` (CLI, served mutation, remote Machine routing): same wire
  shape, `cwd` unchanged in the answer. No DTO field changes.
- `lf resume` with no ID in an unowned linked worktree: now finds a bound
  conversation that runs there.
- Desktop **Bind to Task** / **Make primary Task conversation**: copy at
  `swift/LoopflowMac/Views/SessionActionSheet.swift:75` loses its two sentences
  about the directory. Check that the Task workspace opens the conversation
  when its directory is not the Task's worktree (the done-Task-without-checkout
  case already does this).
- Every harness: the directory is harness-independent. The test's
  `native-locations.json` assertion shows a resume currently relocates the
  provider's saved conversation to the new directory; find what does that, and
  delete it if binding was its only caller (Machine moves may still use it).

## Absent and error states

- A worktree with no conversation still answers "This worktree has no
  associated work conversation." Unchanged; see `scratch/questions.md`.
- The conversation's worktree was removed: launch reports the missing
  directory, as for any Session today.
- Sessions already moved by a bind keep the rewritten `cwd`. No repair code:
  `lf resume <session>` reaches them from anywhere. The live one is
  `session_94d9963b7fd74dacbe01b65c99f7deb5`; nothing in this Task edits the
  store by hand.

## Exclusions

- Giving a Task a second checkout, or moving files between worktrees.
- `lf task checkout`, Machine moves (`lf session move`, `lf task move`).
- Why the agy process ended at 00:29.

## What goes

- `continue_in_task` parameter and `destination` in `adoption::save`, with the
  "Conversation directory set to …" diagnostic.
- `destination` parameter of `promote_task_prepared` / `store::ownership::promote_task`
  and the `UPDATE agent_sessions SET cwd` with its "Task workspace changed before
  promotion" check; callers in `children.rs`, `children_tests.rs`,
  `admission_lifecycle_tests.rs`, `ownership/tests.rs`.
- The directory sentences in `lf session bind`'s `after_help`
  (`rust/loopflow/src/lf/mod.rs:1829`), `docs/work.md:159-161`,
  `docs/dev/reference-models.md:478-484`, and the Swift copy above.
- In `adoption/tests.rs`: the "sets the conversation's directory there" half of
  `binding_selects_the_task_conversation` becomes "stays in `origin`";
  `binding_a_done_task_keeps_it_done` expects `origin` in all four cases.
- Comments that say binding selected a directory (`primary.rs:337`,
  `sqlite/sessions.rs:1754`) say what is true now: a bound conversation keeps
  its directory.

Stays: `resolve_checkout`'s "Session workspace changed before promotion"
re-read, the `work_source='bound'` guard in `admit_session_workspace`, an open
Task without a worktree adopting the Session's unowned linked checkout.

## Passes

One pass. Start with the CLI test rewritten to the new expectations and watch
it fail on `cwd` and on the no-ID resume; delete the move through storage,
execution, help, docs and Swift copy; run the two commands above, `cargo fmt`,
`cargo clippy --all-targets -- -D warnings`. The Desktop build and view tests
go to gate/CI.

Done when: the demo above continues the agy conversation on Claude in the
worktree it started in.

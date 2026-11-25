# LOO-1101: a conversation stays in the directory it runs in

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

Two code paths can rewrite `agent_sessions.cwd` for a conversation that already
ran in another directory:

1. **`lf session bind` to a Task with an existing worktree.**
   `adoption::save(.., continue_in_task = true, ..)` computes `destination`,
   and `promote_task_prepared` runs `UPDATE agent_sessions SET cwd=?2`
   (`rust/loopflow-execution/src/ops/task/adoption.rs:191`,
   `rust/loopflow-storage/src/sqlite/children.rs:429`). Shipped in #1807
   (LOO-582), which left `lf task create --session` in place while making
   `lf session bind` rewrite `cwd`. No local transcript relocation exists (the
   test's `native-locations.json` is written only by the `opencode_cli.py` test
   stub on launch), so on directory-keyed providers like Claude Code
   (`~/.claude/projects/<encoded-cwd>/`), rewriting `cwd` also breaks native
   `--resume`.
2. **`admit_scope_workspace` / `admit_session_workspace` guarding on leaving
   `work_source='bound'`.**
   `rust/loopflow-execution/src/ops/human_session/primary.rs:338` and
   `rust/loopflow-storage/src/sqlite/sessions.rs:1756` keep a Task primary in
   its directory only when `work_source == 'bound'` — a column already marked
   `-- leaving` (`LOO-836`). If a conversation already belonged to a Task
   (`task_id = Some(T)`, `work_source = 'declared'`, `bound_at = None`, started
   from another checkout or `main`) and is made primary via `lf session bind` /
   **Make primary Task conversation**, `bind_session_in` leaves `work_source`
   and `bound_at` untouched, and the next idle `lf resume` silently rewrites
   `agent_sessions.cwd` to `task.worktree`.

## Design: once a conversation has started or been bound, its directory stays put

Two clean rules replace the `continue_in_task` split and the `work_source='bound'`
special case:

1. **Binding changes what a conversation belongs to, never where it runs.**
   Delete `continue_in_task` and `destination` from `adoption::save`,
   `store::ownership::promote_task`, and `promote_task_prepared`. After
   `lf session bind` (and `lf task create --session`), the Session has its Task
   and is the Task's primary; its `cwd` is the directory its provider runs in,
   unchanged. When the Task already owns a different worktree, `lf session bind`
   prints one factual diagnostic noting both paths:
   `Conversation stays in <session_root>; <TASK>'s checkout is <task_worktree>.`
2. **Workspace admission places only unstarted, unbound primary placeholders.**
   In `admit_scope_workspace` (`primary.rs`) and `admit_session_workspace`
   (`sessions.rs`), gate placement on `session.captured.is_none() &&
   session.bound_at.is_none()` (SQL: `CASE WHEN current_capture IS NOT NULL OR
   bound_at IS NOT NULL THEN cwd ELSE ?3 END`). Fresh primaries created before
   their checkout exists (`ensure`, `ensure_task`, `lf task create --run` /
   `setup::checkout`) have `captured == None` and `bound_at == None`, so they
   still land in their scope's worktree before their first launch. Any
   conversation that has already launched (`current_capture IS NOT NULL`) or
   been bound (`bound_at IS NOT NULL`) keeps its directory — unless the
   directory itself is gone from disk (`leave_gone_directory`) or it moves to
   another Machine (`reopen_moved_session`).

What follows across layers:

- `lf resume -a claude:fable` in the conversation's worktree finds it through
  `checkout_conversation` and starts the new thread there, seeded from the agy
  log (`invocation_intent` → `NewThread` → `new_thread`).
- `lf resume MTG-388` / `--task` from anywhere opens the same conversation in
  the directory it runs in.
- A restart (recovery pass, take over, another harness, or making an
  already-assigned conversation primary and resuming it idle) launches where
  the files and provider transcript actually are.
- `agent_sessions.cwd` and `processes.cwd` agree again.
- Two production reads of leaving column `agent_sessions.work_source` disappear
  (`primary.rs:338`, `sessions.rs:1756`).

Choices made for review (recorded in `scratch/concept-review.html`):

- **Never move on bind (including out of `main` and Wave checkouts).**
  Passed over: *keep the move and match live process cwd on resume* (adds a
  second resolver and still breaks on first restart / Claude `--resume`), and
  *move only out of shared checkouts* (adds a special rule that diverges from
  `lf task create --session`, still splits live process from `cwd`, and breaks
  Claude `--resume`).
- **Print one factual line on `lf session bind` only when the Task already owns
  a different checkout.** Passed over: *say nothing about directories* — a
  split between conversation directory `Y` and Task checkout `X` means Graph
  workers and `lf task changes` act on `X`, so naming both paths when they
  differ prevents surprise without adding state.

The trade-off: a Task whose checkout is `X` can have its primary conversation
in `Y`. Graph workers, `lf task changes`, PR delivery, and `lf task move` still
operate on `X`; `lf task status` and `lf session bind` report the real paths.

## User-visible outcome

In a worktree where a conversation runs, `lf resume` and `lf resume -a <agent>`
open that conversation whether or not it was bound to a Task that has another
worktree. Binding reports the Task it bound to, and when the Task already has a
different checkout, notes both paths without moving the conversation.

## Demo

```bash
cd ../repo.research && lf -a agy            # a conversation in a Taskless worktree
lf session bind <session> --task LOO-12     # LOO-12 already has ../repo.loo-12
lf resume -a claude:fable                   # same Session, new thread, still in ../repo.research
```

## Source of truth

- `agent_sessions.cwd` is where the conversation runs.
- `task_checkouts.worktree` is where the Task's branch, PR, and Graph workers live.
- Binding writes `task_id`, `work_source`, `bound_at`, and `task_primary_sessions`;
  it stops writing `cwd`.
- `current_capture IS NULL AND bound_at IS NULL` identifies an unplaced primary
  placeholder that `admit_scope_workspace` / `admit_session_workspace` may
  place before first launch.

## Affected surfaces

- **Storage (`rust/loopflow-storage/src/sqlite/`):**
  - `children.rs`: remove `destination` parameter from `promote_task_prepared`,
    its `UPDATE agent_sessions SET cwd=?2`, and its `"Task workspace changed
    before promotion; retry"` check.
  - `sessions.rs:1746`: `admit_session_workspace` guards with
    `CASE WHEN current_capture IS NOT NULL OR bound_at IS NOT NULL THEN cwd ELSE ?3 END`
    (dropping its `work_source='bound'` read).
- **Execution (`rust/loopflow-execution/src/`):**
  - `store/ownership.rs`, `store/children.rs`: drop `destination` from
    `promote_task`.
  - `ops/task/adoption.rs`: drop `continue_in_task` and `destination` from
    `adoption::save`; when an existing Task has a `worktree` whose canonical
    root differs from `source.checkout` (or `session.cwd`'s worktree root), emit
    the one-line factual diagnostic:
    `Conversation stays in <session_cwd>; <TASK>'s checkout is <task_worktree>.`
  - `ops/human_session/primary.rs:322`: in `admit_scope_workspace`, return
    early when `session.captured.is_some() || session.bound_at.is_some()`
    (dropping its `work_source == Some(WorkSource::Bound)` read).
- **CLI & docs:**
  - `lf session bind` (CLI, served mutation, remote Machine routing): same wire
    shape, `cwd` unchanged in the answer. No DTO field changes.
  - `lf session bind` `after_help` (`rust/loopflow/src/lf/mod.rs:1829`): remove
    the directory-switch sentences; state that the conversation stays in its
    current directory.
  - `docs/work.md`, `docs/sessions.md`, `docs/dev/reference-models.md`: updated
    in `write-docs` to state that binding and creation leave a conversation in
    its directory, document the split-checkout bind output and resume behavior
    (unowned worktree vs. shared `main`/Wave checkouts), and update workspace
    admission rules.
- **Desktop (`swift/`):**
  - `swift/LoopflowMac/Views/SessionActionSheet.swift:75`: replace the
    four-sentence split copy with `"This conversation and its files stay in their current directory."`
  - `swift/LoopflowTests/SessionControlsTests.swift`: update `ControlsSource`
    (line 210: stop rewriting `record["cwd"] = "/src/delivery"`) and assertions
    in `createOrBindConversation` (line 98) and `bindAssignedConversation`
    (line 128) to expect `cwd == session.cwd`.
  - Desktop already launches conversation terminals from
    `record.workspace.worktree` (derived from `session.cwd` via
    `SessionsView.swift:1257` `presentationDirectory`), so a Task whose primary
    conversation runs in another checkout opens in the conversation's directory.
- **Harnesses:**
  - Harness-independent. Keeping `cwd` unchanged also preserves directory-keyed
    transcripts (Claude Code's `~/.claude/projects/<encoded-cwd>/`). No local
    relocation code exists to delete (`native-locations.json` is only written
    by `rust/loopflow-execution/tests/support/opencode_cli.py`).

## Absent and error states

- A worktree with no conversation still answers `"This worktree has no
  associated work conversation."` Unchanged; see `scratch/questions.md`.
- The conversation's worktree was removed from disk (`!session.cwd.is_dir()`):
  `leave_gone_directory` (`primary.rs:288`) falls back to the repository's main
  checkout when a repository is recorded on the Task, Wave, or Session; with no
  repository on record, launch reports the missing directory.
- Sessions already moved by a past bind keep their recorded `cwd`. No repair
  code: `lf resume <session>` reaches them from anywhere. The live one is
  `session_94d9963b7fd74dacbe01b65c99f7deb5`; nothing in this Task edits the
  store by hand.

## Exclusions

- Giving a Task a second checkout, or moving files between worktrees.
- `lf task checkout`, Machine moves (`lf session move`, `lf task move` — note
  that `moves::send_task` still checkpoints `task.worktree`, separate follow-up
  if split-checkout moves need warning/refusal).
- Changing `lf resume` in an empty unowned linked worktree.
- Removing `agent_sessions.work_source` completely (`LOO-836`) or merging
  test-only `bind_session_prepared`.
- Why the agy process ended at 00:29.

## Checkout adoption alongside resume

Keeping a bound conversation in its worktree also leaves it visible to
`lf task create --worktree … --run`, which shares the last-opened Session
lookup with resume. A Task adopting this checkout must select the last
conversation eligible to belong to it, or create its own. Resume still sees
the bound conversation. Check both readings with two Sessions in one checkout.

## Implementation and verification

- Binding commits Task assignment and primary selection without a directory
  update. Only unstarted, unbound primary placeholders take their scope’s
  checkout. CLI help and the Desktop binding sheet describe the same rule.
- The native resume fixture checks the live attachment, Task selection, lookup
  from the unowned checkout, provider cwd, native history and the untracked draft.
  The provider-switch fixture runs `lf resume -a claude:fable` from that
  checkout using the agy and Claude stand-ins.
- SQLite query plans and warm in-memory timings on the Machine schema with
  10,000 synthetic Sessions: workspace admission used the Session primary key
  and the unique primary mapping (2.9 µs median, excluding commit).
  With 1,000 unfinished conversations, checkout selection used
  `session_inventory` (0.41 ms median for resume, 0.44 ms for adoption).

## Status

All surfaces above are implemented (commit e1a15bfcf3); docs match (9caf7d133c).
Nothing remains to build. Checks:

- `cargo test -p loopflow-execution --lib ops::task::adoption::` after merging
  41023c8e55: 11 passed (realign, 2026-11-25).
- Native resume and provider-switch fixtures in
  `rust/loopflow/tests/session_launch_contract.rs`,
  `session_lifecycle_tests.rs`: run at gate/CI.
- Desktop build and `SessionControlsTests`: gate/CI.
- Open review choices stay in `scratch/questions.md`.

# LOO-596: the unbreak step ended with its merge half-done (2026-10-25)

## What failed

Graph `incident` (`8f8ad305`) ran `unbreak` → `5whys` → `launch-plan` on LOO-596.
The unbreak step (lf Process `5f761895`, Session `session_152a99b9…`, Claude
conversation `653a8453…`) merged `origin/main` into #1652's head on local branch
`loo-596-sync`. It resolved every conflict, built, passed clippy, and then ended
its turn with:

> Integration suites are still running; I'll pick up when they report.

Its lf Process exited `succeeded`/0 at 22:29:18 and the Graph moved to `5whys`.
Fifteen seconds after the turn ended, Claude enqueued two `task-notification`s
for the two background commands, both `killed`.

Affected: LOO-596 and PR #1652 (LOO-561). Jack's merge decision is implemented in
the working tree but unproven and unpublished.

## Recovery status (not repaired)

Observed in this checkout at the start of `5whys`:

- `git status`: "You have unmerged paths." `rust/loopflow/src/ops/pr_landing.rs` has
  no conflict markers but its index entry is still unmerged (`UU`); the merge
  is not committed. `task_execution.rs`, `migrations.rs`, `error.rs`, `pr.rs`,
  `task.rs`, `graph.rs` and `tests/fixtures/dto/task_lifecycle.json` have unstaged edits.
- Unit run `bnk1o9dk1`: 163 passed, 2 failed
  (`an_interrupt_ends_the_running_agent_step…`, `ordinary_task_comments_and_completion…`).
  The agent then edited the lifecycle fixture (`moving` → `shipping`, three
  lines) and reported "Both pass". Clippy reported clean.
- The integration suites (release, landing, PR, ship, DTO) never finished. Their
  outputs say `[killed]`.
- Nothing pushed; #1652's description not updated (steps 4–5 of the Task).

The next step that does the Task's work must stage `pr_landing.rs`, rerun the
integration suites, commit the merge, push fast-forward and edit #1652. Analysis
did none of this.

## Causal chain

1. **The step ended before the work was done.** The agent's last message
   (`end_turn`) said it was waiting for background tests. Evidence: transcript
   tail of `653a8453…jsonl`, 22:28:33.

2. **Why did the agent end its turn while waiting?** It had run the suites with
   `run_in_background` and believed it would be re-invoked. Claude Code's own
   contract says so: the Bash tool "keeps running across turns and re-invokes you
   when it exits", and Claude did enqueue the notifications for a new turn
   (`queue-operation enqueue`, 22:28:49). Under Claude Code alone, that belief holds.
   The suites took longer than the 10-minute foreground Bash cap (the unit run alone
   took 594 s), so backgrounding was reasonable.

3. **Why did no new turn arrive?** Loopflow treats the first `result` as the end of
   the step. `ClaudeHarness`'s reader (`rust/loopflow/src/harness/claude.rs`, ~L293)
   emits `TurnCompleted` when `pending_results` drains on a `result` line. The batch
   runner then settles the step and shuts the process down. The background shells
   die with it, and so does the queued notification. lf counts results it caused
   (seed + steers), not turns the provider starts on its own.

4. **Why did the instructions not warn the agent?** `builtins/surfaces/headless.md`
   says "keep moving" and "explain the failure… and stop", but not that ending
   the turn ends the run. The system prompt the agent sees promises a re-invocation
   lf doesn't honor. The two contracts disagree, and nothing tells the agent which wins.

5. **Why did the Graph advance as though unbreak succeeded?** A step's result is
   how its Process exited (AGENTS.md: "A step's result is how its process exited").
   The agent turn completed without error, so the Process exited 0. The unmerged
   index, killed suites and missing push left no trace in the exit. This is the
   intended model and not a defect by itself. It only means (3) is invisible
   downstream.

### Same shape seen before

The infrastructure wave memory records Pi's version of this: "`agent_end` may precede
retry; `agent_settled` ends the Loopflow turn". The provider's turn-end event is not
the same as the provider being settled. Loopflow fixed that for Pi and not for
Claude's background tasks.

## Unanswered questions

- Does Claude's `stream-json` stdout carry an event for background tasks starting
  or finishing (e.g. a `system` subtype), or only the on-disk transcript's
  `queue-operation`? The lf fix depends on this. Not checked against a live stream.
- Was LOO-586's lost partial resolution the same failure? Its Task says it "stopped at
  `pr_landing.rs`" and the resolution "is gone". The `sync-1652` branch it left
  in this repo is the bare PR head (`b5058a91d1`) with no merge commit, which fits
  uncommitted merge work discarded with its worktree. Its transcript was not read here.
- Codex and OpenCode: do they have backgrounded work that outlives a turn? Not checked.

Contradicting evidence: none found. The agent did not misjudge the headless rules
as written; it followed the provider's contract.

## Prevention (proposed, not built)

1. **Settle on the provider being idle, not on the first `result`** (code, highest
   leverage). In `ClaudeHarness`, a batch step keeps the process alive while
   backgrounded tool calls are outstanding, and closes the turn on the `result`
   that follows their notifications. This is the Pi `agent_settled` lesson
   applied to Claude. Proof: a stand-in `claude` fixture
   (`harness/claude.rs` already scripts one, ~L1014) that emits `result`, then a
   notification-driven second turn with the real answer. Assert that the step's
   final answer is the second one and that the process was not killed between.
   Open question 1 decides whether the reader can see the background work at all.
   If it can't, see 2.

2. **Say it in `headless.md`** (prompt, cheap, works today). Add one line: ending
   your turn ends the run, so background commands do not report back. Wait for
   them in the foreground with a polling loop under the tool timeout before you
   answer. This brings the two contracts into line now. It is a reminder, not a
   structural fix, so keep it even after 1 lands only if 1 can't see every kind
   of background work. Proof: the existing `prompt/mod.rs` headless assertions
   cover its presence. The real test is the next long gate or merge step that
   finishes in one turn.

3. **Not proposed:** a post-step guard that refuses exit 0 with an unmerged index
   or dirty tree. It would be the kind of check AGENTS.md's Flexibility section
   argues against, and it would only catch git-shaped symptoms of (3).

Useful next action: whoever continues LOO-596 finishes the merge as listed under
Recovery. Separately, a small Task for prevention 1 (with 2 as the stopgap) on
the infrastructure wave.

# LOO-586: keeping #1652 and #1671 mergeable

## #1671 (jack-heart/new-marketing): repaired
- Merged origin/main as a601c6211d, pushed fast-forward (7e64823624..a601c6211d); auto-merge left as it was.
- README conflict: the branch rewrote the README and moved the Sessions docs out of it. Took the branch README; carried main's #1674 intent (connect by Task identifier; `detached` Sessions reattach) into docs/lf.md. Main's #1675 `task run` line wasn't in the new README.
- documented_commands failure: the prose `"Use lf to split..."` parsed as `lf to` (ambiguous). Reworded to `"Split this into Tasks and run them with lf."`. `cargo test -p loopflow --test integration documented_commands`: 4 passed.
- Status 10:05 UTC: MERGEABLE, auto-merge armed, CI running on a601c6211d. Not yet proven green.

## #1652 (LOO-561): NOT repaired, left DIRTY and unarmed
Conflicts: graph.rs (#1678 interrupt), task_automation.rs and ci_watch.rs (deleted on the branch, modified on main), release_lock_tests.rs, pr_landing.rs (21 hunks, #1679/#1680).
- graph.rs, task_execution.rs (main's `unresolved_processes` moved into the branch's home), task.rs and release_lock_tests.rs were resolved; saved as `scratch/sync-1652-partial-resolution.patch` (diff against the branch head, uncompiled).
- pr_landing.rs is a policy conflict, not a text conflict. The branch classifies a dropped merge request as `Dequeued { reason }` and re-arms it in-process (`RearmPr`), including after a manual removal, and reads its own timeline `merge_exit`. Main's #1680 (merged) uses `decide_merge` plus `merge_request_removal`: a person's removal → `Withdrawn` blocks; evictions or a Loopflow disarm → failing checks to ci-fix. A merge keeps one reader and one policy. LOO-561's plan reserves "manual-dequeue policy" for Jack's re-demo, and its conversation holds Jack's unsent direction about this behavior. That conversation should do the merge.

## First root cause (superseded below)
Integration repair runs only inside a Loopflow landing (`merge_needs_integration` → `required-integration` → ci-fix). Unarmed PRs and PRs armed straight on GitHub have no owner while their conversation is idle. Filed LOO-587 (infrastructure): the Task operator / `lf task reconcile` re-syncs DIRTY idle Task PRs, not the CI watcher (#1652 deletes `lf ci watch`).

---

# Five whys (2026-10-25)

The two PRs failed for different reasons. Neither matches the first root cause
exactly: #1671 had an owner that declined to act, and #1652 has a conflict no
automatic re-sync could resolve.

## #1671: an armed landing whose repair deferred to idle Sessions for five hours

**What failed.** Jack's README rewrite was armed with `lf land` at 04:40 UTC and
sat failing, then DIRTY, until LOO-586 merged main by hand at 10:02.

1. *Why did it not merge?* Its first CI run (04:40, head 7e64823624) failed
   `rust-test (integration, 2, 3)`, which is `documented_commands`. The defect
   was the branch's own prose (`lf to`), not something main introduced. Main then
   moved and the PR also went DIRTY.
2. *Why was the failure not repaired?* The repair owner existed and saw it.
   `pr_landings` holds `landing_defa437…` for #1671 (taskless, state `watching`).
   `ci_incidents` holds two incidents on 7e64823624: the integration test (observed
   04:43) and `required-integration` from DIRTY (observed 05:59). Both have
   `claimed_landing_generation`, `responded_at` and `repair_lf_process_id` null.
   The landing's generation reached 175: the watcher re-entered
   Watching → Repairing → Watching on every pass and never started ci-fix.
3. *Why did every attempt back out?* `admit_ci_fix`
   (`rust/loopflow/src/ops/pr_landing.rs:335`) refuses a taskless landing when
   `checkout_execution_blockers` finds any open Process in its worktree that is
   not known dead; `CheckoutBusy` puts the landing back to Watching
   (`pr_landing.rs:1077`). `../loopflow.new-marketing` holds two live interactive
   Claude Sessions started on Oct 24 at 19:20 and 19:22 PDT: `default` (pid 24260;
   the conversation that ran `lf land`) and `design api-for-agents` (pid 89919).
   Both were idle. Nine uncompleted Processes there, all alive.
4. *Why does an idle Session block repair?* `execution_blockers`
   (`ops/task_automation.rs`) asks only the OS: any open Process outside the
   caller's lineage that isn't dead blocks. Liveness stands in for "someone is
   editing this checkout". An open terminal waiting for input is indistinguishable
   from an agent mid-edit. The same rule applies to Task landings through
   `task_execution_blockers`, so this is not specific to taskless PRs: the
   normal path, landing from your conversation and leaving it open, defers every
   repair until that conversation exits. The rule dates from #1499/#1512
   (2026-10-09).
5. *Why did nobody notice for five hours?* The only signal is the watcher's
   per-PR detail (`repair waits: Process … has live or unresolved execution`).
   It isn't raised to the conversation holding the checkout or to a person, and
   it has no elapsed bound (LOO-503 removed elapsed-time cutoffs on purpose).
   Generation churn is invisible.

**Branch.** Repair runs ci-fix inside the landing's own worktree, which is why it
must wait for other writers. That constraint is real: two agents editing one
checkout corrupts work. The choice is between letting repair work elsewhere and
telling the Session that holds the checkout.

## #1652: a policy fork that re-syncing cannot fix

**What failed.** LOO-561's PR has synced with main three times since Oct 24
(936afed62c, b1120743c1, a3a4268c06) and was DIRTY again by 08:07 UTC. It is
unarmed by design; Jack merges it.

1. *Why does it conflict again?* Four main PRs after its last sync touch its
   files: #1678 (graph.rs, task_automation.rs), #1679 and #1680 (pr_landing.rs,
   ci_watch.rs, pr.rs), #1681. The PR changes 201 files (+7839 −5295, 80 commits,
   open since Oct 22), so nearly every main PR overlaps it; all ten merges
   from #1668 to #1681 touched at least one of its files.
2. *Why is the pr_landing.rs conflict not mechanical?* Four incident Tasks in
   seven hours rewrote the rule for a dropped merge request: LOO-573 (#1668, a
   manual dequeue stops a release), LOO-577 (#1673, lf's own disarm isn't a
   person's), LOO-583 (#1679, an eviction goes to ci-fix), LOO-584 (#1680, one
   shared decision: only a person's removal blocks). LOO-561 rewrote the same
   code toward re-arming after any removal, and its plan reserves the
   manual-dequeue policy for Jack at re-demo (commit 863e6793e0).
3. *Why did the incident Tasks not account for LOO-561?* None of the four
   descriptions mention LOO-561 or #1652. They were filed from live incidents by
   the repository conversation, and each fix was right for its incident.
   `capture-tasks` points at `lf wave status` for overlap, which lists plans,
   not which open PRs rewrite the same files. Nothing in Task creation or
   `lf task run` reports "an open PR changes this code".
4. *Why could a long branch absorb that much drift?* LOO-561 is one Task = one
   PR covering ship, Workflow states, ci_watch deletion and landing policy. Each
   sync is a full agent pass, and main moved faster (ten merges overnight) than
   an idle interactive conversation re-syncs.

**What contradicts "nobody owns re-sync".** #1652's conversation did re-sync,
three times. An automatic owner (LOO-587) would have hit the same pr_landing.rs
fork and stopped. Main's #1668 was itself caused by Jack dequeuing a release on
purpose, which favors "a person's removal blocks", main's current behavior. That
is evidence for the decision, not the decision; it remains Jack's.

## Open questions
- Did anything surface `repair waits` for #1671 between 04:43 and 10:02? The
  watcher state at that time wasn't captured; the inference rests on the store
  rows and live pids above.
- Is the `api-for-agents` Session in the new-marketing checkout intentional? A
  second conversation in a delivery checkout blocks its repair just like the
  first.
- Will the merged #1671 head go green? CI was still running at 10:05 UTC.

## Proportionate prevention (proposed, not approved)
1. **Tell the holder, not the log (#1671).** When `admit_ci_fix` returns
   `CheckoutBusy` because of an interactive Session, deliver the incident into
   that Session as its next turn, or mark it Available with the failing checks,
   so the conversation that ran `lf land` repairs its own PR. Prove it with a
   landing fixture: an idle attached Session in the checkout plus a failing check
   ends with the Session receiving the incident, not a generation counter
   climbing. Alternative to weigh: ci-fix in a fresh worktree on the PR branch,
   which removes the wait but forks the branch under a live conversation.
2. **Count it.** Generation 175 with no claim is a loud signal hidden in a
   column. The telemetry cron (`telemetry-daily`) could flag a landing whose
   generation rose N times without a response. Cheap, independent of 1.
3. **Correct LOO-587's premise.** It states that an armed PR has no owner when
   its conversation is idle. For #1671 the owner existed and deferred. Re-sync of
   unarmed Task PRs (#1652's shape) still has no owner, but for #1652 that owner
   would have stopped at the same policy fork. LOO-587's demo should include a
   conflict it must hand back.
4. **Overlap at filing time (#1652).** Before a Task is filed against files an
   open PR rewrites, name that PR and its Task in the brief, e.g. from
   `gh pr list` plus `--name-only` diffs in capture-tasks. This would have put
   "LOO-561 reserves this policy for Jack" in front of LOO-577/583/584. Prompt
   change, provable only by review of the next incident brief.
5. **Not proposed:** splitting LOO-561 now. Its re-demo is pending with Jack;
   that conversation owns the merge and the policy choice.

# Launch plan (2026-10-25)
- **Here (done):** proposal 4. `capture-tasks` and `launch-plan` now list open PRs
  that change the files a new Task targets (`gh pr list --json ...,files`) and
  name each PR, its Task and any choice it reserves for a person in the brief.
  Run against today's tree, it returns #1652 for `pr_landing.rs`, the warning
  LOO-577/583/584 lacked. Prose-only builtin change; no build.
- **Filed, not launched:** LOO-588 (proposals 1+2: repair reaches the Session
  holding the checkout; repeated deferral visible). It rewrites code #1652
  rewrites or deletes, so it waits for #1652; transport choice is Jack's.
- **Refined:** LOO-587 (proposal 3). Premise narrowed to unarmed Task PRs, demo
  must hand back a policy conflict, #1652 overlap named; comment posted.
- **Not done:** merging main into #1652 (still DIRTY; LOO-561's conversation owns
  the policy fork; partial resolution in `sync-1652-partial-resolution.patch`).
  #1671 MERGEABLE/BLOCKED with swift-test still pending at last check.

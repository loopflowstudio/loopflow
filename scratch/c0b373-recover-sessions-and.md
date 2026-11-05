Jack Heart requested on November 5, 2026 that the remaining account/model-limit recovery half of LOO-714 become a new Task, be launched, and that LOO-714 be completed after handoff. The settings half is already merged, installed and verified (PR #1805; installer fix #1824; installed 0.15.1+5519340b4).

Outcome
Keep work moving when an account or model runs out of usage, without making Jack repair every open Session or restart each failed Graph manually.

Accepted scope
1. Interactive Task Sessions: a Session whose account/model hits a usage limit can continue on eligible remaining capacity while preserving its durable conversation and work. Multiple open Sessions should not each require manual account/model repair. Jack described the desired interaction as one place, or no place, to recover them.
2. Headless Graph steps: a limit must lead to a continuation/retry of the affected step using eligible remaining capacity, preserving the Task checkout, conversation and progress. Do not replay already completed steps or external effects. Wait for reset only when nothing eligible can continue, and expose that waiting state honestly.
3. Both paths share authoritative limit observations and account/model selection. Respect the file-backed preferences shipped by LOO-714, including scoped account order and explicit agent choices. Recovering one invocation must not rewrite personal preferences or leak a conversation's -a override into unrelated Tasks/steps.

Implementation and proof
Start by writing a focused design in this Task's scratch and inventorying the existing failure classifier, account resolver, cooldown observations, Session continuation and Graph retry paths. Keep one owner for each decision; remove conflicting paths rather than adding another selector. Preserve LfSession identity and history; native provider thread reuse versus history import must follow what the provider/account boundary actually supports. Never record a successful account/thread association before the launch really uses it. Avoid disrupting active user input during interactive recovery. Preserve credential secrecy and existing routing/usage attribution.

Acceptance should exercise both interactive and headless paths with realistic provider limit events, exhausted primary/eligible alternate, all accounts exhausted, reset/recovery, and repeated observations/concurrent Sessions. Show that no completed Graph effect is replayed, no work or history is lost, and UI/CLI distinguish retrying, waiting, and terminal non-limit failures. Use deterministic fixtures plus a configured demo; do not intentionally consume paid quota to manufacture exhaustion.

Historical evidence to revalidate against current code
On November 2 a Claude event rejected: seven_day_overage_included put primary in cooldown. The retry loop classified it as a generic transient rate limit and retained the native resume thread, while the account router independently selected loopflow. retry_agent_process rejected that combination with 'AgentProcess retry must retain its thread unless the selected account changes'. Seven runs showed the failure; after both accounts cooled, others failed with 'no eligible managed claude account'. A provider_session_accounts row may have rebound a thread to the alternate account before a successful launch. This is an investigation lead, not a claim that current source still has the same defect.

Open design choices
Define automatic versus one-action interactive recovery, supported model/harness fallback within explicit preferences, and how a waiting Graph survives process exit/restart and resumes at reset. Resolve routine implementation choices autonomously; surface consequential user-facing policy at review. Earlier LOO-714 Route/tier proposals, no-retry-budget claims, and proposed lf route commands were drafts, not accepted APIs. Do not revive them automatically.

Related work and boundaries
- LOO-714 settings resolution and installation are delivered; do not reopen that redesign. This Task replaces its remaining recovery scope.
- a80b23 owns auth/browser config storage and improved account UI. Recovery may expose necessary retry/wait state, but broad account-management cleanup belongs there.
- LOO-324 owns cross-account/Codex-history Session resume; LOO-435 owns generic retry-from-failed-step; LOO-540 owns scoped account selection. Inspect and reuse their relevant work, link overlaps, and avoid duplicate implementations or silently completing their separate outcomes.
- Open PR #1656 / LOO-312 changes agent launch and provider auth for newcomer onboarding. Preserve its pending review choices and coordinate overlapping edits.

Launch the normal code graph, using current resolved agent preferences. Stop at its interactive demo/review boundary; creating and launching this Task does not approve Auto-Ship.

# Design (November 5, 2026; built, awaiting demo/review)

## Inventory, revalidated against 3b07e13404
- Limit observation: decoders emit `RateLimitSignal`; `FactWriter` → `route.record_rate_limit` → `record_rate_limit_signal` writes `provider_accounts.cooldown_until` + windows. One writer; keep it. Terminal Claude emits no stream, so lf never observes its limits today.
- Classifier: harness turns a limited signal into Error `provider_rate_limited: rejected: <type>`; `_classify_agent_failure` re-read that text, missed the subscription patterns and matched `rate_limit` → generic `RateLimit`. The Nov 2 defect was live at the base; decision 1 removes it.
- Retry: `_run_with_transient_retries` keeps the thread on RateLimit; `_run_harness_once` re-selects; `retry_agent_process` bails when account changed with a thread. Failover drops the thread instead (conversation lost). Failover shares the transient budget.
- Selection: `select_route`; failure is `NoEligibleAccount` (string only, no reset time). `pin_provider_account_id_blocking` fails before the retry loop when everything is cooling.
- Early association: `resolve_provider_account_exact` → `adopt` → `pin_session` wrote `provider_session_accounts` during selection. Lead confirmed at the base; decision 8 removes it.
- `adopt` already moves transcripts between homes, so same-Machine native thread reuse across accounts is supported (LOO-324's Codex-history import stays separate).
- Graph: a failed step fails the Graph; `lf task run` restarts from where the run began (replays completed steps), 3 attempts. `lf ship` restarts later with backoff.
- Shared mode (Jack's Machine): running Claude follows the native login, so one activation moves every open Claude Session. Codex keeps its login until resumed.

## Decisions
Routine choices are Jack's to overturn at review; 6 and 7 are user-facing policy and the ones to look at first, with `scratch/questions.md`.

1. **Classification is typed.** `provider_rate_limited` sets `AccountSubscriptionLimit`; text never decides for that code. A text-only limit records a default cooldown so failover cannot spin on the same account.
2. **Headless continuation stays inside the step.** The step Process fails over and waits in `_run_with_transient_retries`, so the Graph never sees a failure and no completed step can replay. Failover carries the native thread (adopt), with a short continue prompt; `retry_agent_process` becomes "a retry retains its thread". Limit failovers and waits do not spend the transient budget.
3. **Waiting.** `NoEligibleAccount.retry_at` = earliest cooldown among route candidates; None means nothing will free up, which stays a terminal failure. Wall-clock poll each minute (SystemTime), re-running selection, so a cleared cooldown or a new login ends the wait early.
4. **Truth.** One table, `agent_limit_recoveries` (step Process, provider, reason, retry_at). retry_at set = waiting, null = retrying on another account. `TaskExecutionState::Waiting`; a non-limit failure stays Blocked.
5. **Restart.** A step that dies with a recovery row is where the next `lf task run` starts, and a sweep beside `restart_ships` relaunches it. Completed steps stay completed.
6. **Interactive: no place.** Claude `StopFailure` (`rate_limit`) hook records the limit through the same writer and, in shared mode, activates the next eligible account once. Open Claude Sessions follow; the person's next message continues. No keystrokes injected, so active input is safe. Isolated and Codex Sessions move on their next `lf resume`.
7. **No model/harness fallback.** Preferences have one resolved agent and no ordered alternates, so a fallback would be invented policy. Accounts within the existing route only. Surface at review.
8. **Association.** `adopt` copies only; pin happens when the provider reports the thread under that launch.

## Built (November 5, 2026)
All eight decisions are in the diff. Corrections made while building:
- Waiting is for Graph steps only (`ProcessConfig.wait_for_capacity`); commit and PR text keep failing at once, and release notes keep their fast degrade.
- A Graph step is read from Process ancestry: transport-served sessions lose `LF_GRAPH_ID`. Recovery rows are read through the same delegation chain as captured inputs.
- A thread with no transcript cannot move, so failover falls back once to a fresh thread with the whole task. `retry_agent_process` allows exactly that.
- Interrupting a step ends its wait, so the sweep does not restart it.
- Failovers are capped at `MAX_ACCOUNT_FAILOVERS` per wait: a named account skips cooldown checks, so it could otherwise spin.
- The restart sweep starts from due recovery rows and `task_of_process`, so a stale row costs one Task lookup and idle reads scan nothing. A restarted run leaves its old row until the Process is pruned.
- A provider's words cool an account only on a failed exit; stderr from a turn that succeeded proves no limit.

## Coordination
- Open PR #1656 (LOO-312) edits `agent/mod.rs` near `format_timeout`/`check_cli_available` and `bin/lf.rs`; the retry loop, pinning and the `limit` hook arm are elsewhere, so the merge is textual, not semantic.
- `restart_held_runs` and `restart_ships` share `start_lf_in`; a shipping Task's held wait goes through ship's own restart.

## Remaining
- Restart after a step dies mid-wait starts that step in a new Run: checkout and completed steps are kept, the step's own conversation is not. Resuming it needs the Graph to open the step with `session resume`; LOO-435 is the likely owner.
- `limit_recovery::held` finds a step by name, so a skill repeated in a Graph restarts at its first occurrence.
- Native Claude limits use the 15-minute default cooldown: `StopFailure` carries no reset time.
- Codex terminal limits are observed only through the relay's stream, which keeps its login until resume.
- Demo: configured path not run; no paid quota may be spent. `LIMITED_CODEX` in `tests/agent_tests.rs` is the deterministic stand-in.
- `StopFailure` was confirmed from Claude 2.1.318's bundled schema, not from a live limit.

Checks: implement: focused lib (103), `agent_tests` limit/exhausted/revoked and the held/stopped-run tests pass with isolated `HOME`; `a_batch_task_run_runs_straight_through…` blocks in macOS `security -i` under a fake `HOME` (keychain login write, a path this branch leaves alone), deferred to gate with the Swift build and affected suites. Realign (Nov 5): the wait log now prints the reset as a readable time like Task status does; `cargo test -p loopflow --lib agent::tests::` passed (31). Concept-review fixes (Nov 5): `--lib limit_recovery task_execution:: agent::` (37), both task_graph_launch restart tests, `agent_tests` limit/exhausted with isolated `HOME`, clippy clean. Compress (Nov 5): `restart_ships` doc restored above its fn (`start_lf_in` had taken it), boundary `NoEligibleAccount` formatted; `--lib limit_recovery task_execution:: agent:: ops::task::ship` (40) and clippy clean.
Realign after concept review (Nov 5): all three fixes verified in code; `docs/subscriptions.md` restart row now promises an automatic restart only under `restarts_by_itself`; `--lib limit_recovery task_execution:: agent::` (37), the two `task_graph_launch_tests` restart tests via `--lib` (2), and `agent_tests` limit/exhausted/revoked run as the built binary from an empty directory with isolated `HOME` (3) all pass.

## Concept review (November 5, 2026, headless)
The concepts hold: an account cools, a step goes on under another or waits, and status reads Running, Waiting or Blocked. Its three fixes are in the checkpoint commits before the second realign: the restart contract (AGENTS.md, architecture reference, `docs/subscriptions.md`) names the one exception, a step that ended waiting on a usage limit; `limit_recovery::waiting_on_limit` / `restart_waiting_runs` carry the Waiting name; `restarts_by_itself` (assigned, unfinished, no execution blockers) decides both the sweep and whether status or the user doc promises an automatic restart, otherwise status names `lf task run <issue>`.

Not blocking here: `scratch/questions.md` 1–5 are user-facing policy the brief sends to the demo/review step (overage cooling a whole account, unbounded waits, no model/harness fallback, interactive continuation on next message, isolated resume no longer exact). They stay open for Jack there.

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
Full inventory in git (`git show d8d6b33eb:scratch/c0b373-recover-sessions-and.md`). What still matters: the Nov 2 classifier defect (subscription limit read as generic `RateLimit`) and early `provider_session_accounts` association were both live at the base and are removed by decisions 1 and 8; in shared mode one activation moves every open Claude Session, Codex keeps its login until resumed.

## Decisions
Routine choices are Jack's to overturn at review; 6 and 7 are user-facing policy and the ones to look at first, with `scratch/questions.md`.

1. **Classification is typed.** `provider_rate_limited` sets `AccountSubscriptionLimit`; text never decides for that code. A text-only limit records a default cooldown so failover cannot spin on the same account.
2. **Headless continuation stays inside the step.** The step Process fails over and waits in `_run_with_transient_retries`, so the Graph never sees a failure and no completed step can replay. Failover carries the native thread (adopt), with a short continue prompt; `retry_agent_process` becomes "a retry retains its thread". Limit failovers and waits do not spend the transient budget.
3. **Waiting.** `NoEligibleAccount.retry_at` = earliest cooldown among route candidates; None means nothing will free up, which stays a terminal failure. Wall-clock poll each minute (SystemTime), re-running selection, so a cleared cooldown or a new login ends the wait early.
4. **Truth.** One table, `agent_limit_recoveries` (step Process, provider, reason, retry_at). retry_at set = waiting, null = retrying on another account. `TaskExecutionState::Waiting`; a non-limit failure stays Blocked.
5. **Restart.** A step that dies with a recovery row is where the next `lf task run` starts, and a sweep beside `restart_ships` relaunches it. Completed steps stay completed.
6. **Interactive: no place.** Claude `StopFailure` (`rate_limit`) hook records the limit through the same writer and, in shared mode, activates the next eligible account once. Open Claude Sessions follow; the person's next message continues. No keystrokes injected, so active input is safe. Isolated and Codex Sessions move on their next `lf resume`.
7. **No model/harness fallback.** Preferences have one resolved agent and no ordered alternates, so a fallback would be invented policy. Accounts within the existing route only. Surface at review. Superseded at demo (Accepted at demo, item 3).
8. **Association.** `adopt` copies only; pin happens when the provider reports the thread under that launch.

## Built (November 5, 2026)
All eight decisions are in the diff. Corrections made while building:
- Waiting is for Graph steps only (`ProcessConfig.wait_for_capacity`); commit and PR text keep failing at once, and release notes keep their fast degrade.
- A Graph step is read from Process ancestry: transport-served sessions lose `LF_GRAPH_ID`. Recovery rows are read through the same delegation chain as captured inputs.
- A thread with no transcript cannot move, so failover falls back once to a fresh thread with the whole task. `retry_agent_process` allows exactly that.
- Interrupting a step ends its wait, so the sweep does not restart it.
- Failovers are capped at `MAX_ACCOUNT_FAILOVERS` per wait: a named account skips cooldown checks, so it could otherwise spin.
- A restarted run leaves its old recovery row until the Process is pruned.
- A provider's words cool an account only on a failed exit; stderr from a turn that succeeded proves no limit.

## Coordination
- Open PR #1656 (LOO-312) edits `agent/mod.rs` near `format_timeout`/`check_cli_available` and `bin/lf.rs`; the retry loop, pinning and the `limit` hook arm are elsewhere, so the merge is textual, not semantic.
- `restart_waiting_runs` and `restart_ships` share `restart_lf_in`; a shipping Task's wait goes through ship's own restart.


## Accepted at demo (Jack Heart, November 5, 2026)
Supersedes decision 7 and answers `scratch/questions.md` 1–2; the reasoning and evidence are there. Build all of this in PR #1828, with no stacked Task.

1. **Login switch reaches open Claude Sessions.** Live test: a running Claude 2.1.318 re-reads the Keychain only when `<config>/.credentials.json` changes mtime or is absent. `activate` updates that file's mtime (when present) after writing the Keychain. Jack's stale stub in `~/.claude` was removed. Decision 6 then holds; keep it for shared Sessions only. Isolated Sessions never move by themselves.
2. **Scoped limits.** A limit records its scope (all models or one model, e.g. Fable). Cooldown, room ranking and blocked-ness apply only to launches that scope affects; a Fable limit leaves Opus/Sonnet on that account usable. Stream decoding keeps the provider's scope rather than collapsing to `weekly`/`session`; unknown limit types become their own window, never a fixed list.
3. **Fall-through for headless steps.** Order: another account, then another model, then another provider, then wait. An explicit `harness:model` stays on that model (accounts only). Bare `claude` or `codex` may also change model within that provider. Only preference-resolved agents (no explicit agent) may change provider, within an ordered preference list. A provider change continues through the existing history import (`lf resume -a` path), not a fresh start.
4. **Bulk moves for interactive Sessions.** One typed Session selector, spelled as flags, shared by `lf session list` (the preview) and actions: `--blocked` (the Session's next message would fail, derived from its recorded account/model plus scoped limits) and `--on <account|harness[:model]>`. `lf session move <selector> -a …` moves the selected Sessions; it requires a selector. Interactive Sessions only; the command filters explicitly. Moves never write preferences. Desktop gets the same action (Linear-style filtered view, select, act). A query string is not planned. `session move --account` and `lf account drain` were accepted here and deleted November 6 (Concept review with Jack).
5. **No provider pins in Loopflow.** Before landing, nothing built-in or in this repo's `.lf/config.yaml` requires Claude or Codex; remove the five `skills.*.agent` pins (including `xor-route: claude:sonnet`; Jack sets cheaper models as personal preferences if wanted).
6. **Route by soonest reset (Jack, November 5).** Superseded November 6 by quota left per hour until reset (Concept review with Jack).

## Build plan for the demo decisions (November 5, 2026)
Owners, one each. Routine choices are recorded in `scratch/questions.md` 6–10.
- **Login switch**: `activation::write_login` touches `<native>/.credentials.json` after a Claude write, when present.
- **Scoped limits**: `RateLimitSignal.model` (None = every model). Windows keep the provider's `rate_limit_type` verbatim; a `*_opus|sonnet|haiku|fable` suffix is that model's scope. Account-wide limits still cool `provider_accounts.cooldown_until`; a model-scoped limit cools only its window row (`provider_account_limits.model`, `limited_until`, added in this Task's migration). `account_route_eligible` and `tightest_window` take the launch model; a launch with no model is held back by every scope.
- **Fall-through**: `AgentConfig.fallback_agents` carries the rest of a `default_agent` list (`lf set default_agent claude,codex`) only when no explicit agent applies. `_run_with_transient_retries` climbs a ladder on `NoEligibleAccount` with a reset: same agent other model (bare harness only; Claude `opus, sonnet, haiku`), then the next preference agent (history import through `conversation_log::continuation`), then waits. An explicit `harness:model` has no rungs.
- **Selector**: `SessionFilter.blocked` / `.on`; `SessionRecord.blocked` (DTO, fixtures in both languages). Blocked = the Session's account (pinned, else the provider's active login) cannot take its model now.
- **Moves**: `lf session move (--blocked|--on <x>) -a <agent>` in `ops::human_session::moves_bulk`; an agent or model change goes through `new_thread`, a seeded thread (questions 14). Busy Sessions move at their next Stop hook (`session_moves`, applied by `__context-block stop`); no draft signal exists (questions 12).
- **Ranking**: `route_rank` / `urgency` / `order_accounts_by_urgency` (Concept review with Jack).
- **Desktop**: View → Move Blocked Sessions… runs `Write::SessionMove` (`lf session move --all --blocked -a <agent> --json`) and shows the `SessionMoveReport`. The Blocked filter waits for a Sessions list surface; the navigator is Task-first and shows one status per row.
- **Pins**: the five `skills.*.agent` entries leave `.lf/config.yaml`.
7. **Delete isolation everywhere (Jack, November 5).** Each Machine has one shared provider home and one login at a time, switched by lf; load balancing comes from soonest-reset routing over time, not concurrent homes. Remove `--isolate`/`--shared`, `isolate:` config, `LF_ACCOUNT_ISOLATION`, isolated routing (`RouteHome::Isolated`, `provider_session_isolated`), and the restart-based isolated move. `lf ssh` launches use the target's shared home. Account profiles under `~/.lf/accounts/` remain only as stored logins to switch between, never as launch homes. Existing isolated conversations (about 261 Claude, 3,317 Codex) still resume: their transcripts are adopted into the shared home on resume. `.lf/config.yaml` may still carry `isolate:` from the released lf: keep it readable and ignored, with a removal Task. Research and reasons: `scratch/questions.md`.
8. **Remote logins (Jack, November 5).** Keep laptop-driven `lf account connect --machine`: the laptop logs in and sends the remote its own independent login, never a copy of the laptop's. Source already does this (`machine_credentials.rs`: Claude mints a long-lived token, Codex `fresh_machine_login`, OpenCode a fresh login), so the change is only where it lands: the target's shared home and stored-login catalog instead of an isolated launch home.
   Additions (Jack, November 5): the laptop can see which remote logins need refreshing, as `lf account` does locally (health, expiry, capacity per Machine). Renewal happens only out of band (an explicit command) or as a prefix step before a launch (renew if missing or stale, today it only checks missing), never as live credential forwarding while a Session runs; keep "no broker, forwarded socket or laptop process owns the installed login".
9. **No long-lived tokens (Jack, November 5).** Every Claude login is a full login, so features such as `/usage` keep working (a setup-token is inference-only). Remote Claude gets a fresh independent login, as Codex does (`fresh_machine_login`), and refreshes it itself; the laptop's status and prefix renewal cover it. Delete `lf account token`, token minting during connect, and the long-lived credential type and store rows (staged per the destructive-migration rule if a column must go). Accepted cost: each new rental needs a browser approval when connected.
   Refinement (Jack, November 5): an owned Machine (the mini) works like local: the laptop runs a fresh login for it and places it on the Machine, which then holds and refreshes it as its own. Rentals: Jack proposed one laptop-held rental grant, refreshed on the laptop, handing each rental an access token (the Linear pattern in `machine_credentials.rs`). It must outlast the rental (`rental::MAX_HOURS` = 24, renewed only as a prefix step, never live). Claude's access-token lifetime is unverified: if it is under 24h, either shorten rentals to it or connect rentals with a fresh login (one approval each). Settle the lifetime from observed `expiresAt` before choosing; do not print credentials.
   Rental refresh (Jack, November 5): the laptop refreshes rentals' access tokens itself, as a scheduled out-of-band job that rewrites the rental's credential file, not as part of any Session; running Claude adopts the new token when the file changes (mtime test, macOS; Linux file-backed path unverified). Token lifetime then bounds only how long a rental survives the laptop being asleep or offline. Status shows each rental's token expiry.
   Observed (November 5, stored profiles, expiry only): a Claude access token lasts about 8 hours (`primary` written 8.0h before its expiresAt); the long-lived token lasts about a year. So the laptop must refresh rentals at least every few hours, and a rental's Claude stops about 8h after the laptop goes offline.
   Rental choice (Jack, November 5): default is the laptop-held rental login, with each rental receiving only access tokens, refreshed and resent out of band by the laptop every few hours (one token can serve every rental). Optional per rental: treat it as owned, with its own fresh login (one browser approval) so it runs without the laptop. Verify whether a refresh cancels earlier access tokens; if it does, the resend must follow each refresh immediately.

## Built: items 7–9 (November 5, 2026)
- **Isolation deleted.** `RouteHome` is gone: a stored login (`AccountLogin::Stored`) always launches in the shared home; only replay (`AccountLogin::Replayed`) runs in a recorded capture's profile, so this Machine's login never switches to another catalog's account. `--isolate`/`--shared`, `LF_ACCOUNT_ISOLATION`, `launch_isolated`, `prepare_isolated_codex`, `provider_session_isolated`, and the restart-based move are deleted (`names_another_account` and `Departure::Account` came back for Codex on November 7). `isolate:` stays in the catalog as Inactive (read, ignored) until Task e1f359.
- **Legacy isolated conversations.** `provider_session_accounts.isolated = 1` now means only "its transcript is still in that account's home"; `adopt` copies it into the shared home on resume.
- **Remote logins.** Owned Machine: `lf machine connect <m> claude <email>` sends a fresh full login (`fresh_machine_login`, as Codex), installed through `install_managed`. Rental (`rental_for_machine`): the laptop keeps one Claude login per account under `~/.lf/accounts/rental-logins/claude/<id>/`, refreshes it through `subscription::claude_access_token` when within `ACCESS_RENEW_SECS` (2h) of expiry, and sends only `Credential::ClaudeAccess`; after a refresh it resends to every live rental. The rental replaces an older access token and, when its shared home was on it, that too (`activation::renew_claude_login`, mtime marker). `installed` reports an access token stale within 2h, so any connect or launch prefix renews it. `lf --machine <m> account` shows `access token until <time>` (`expires_at`).
- **Long-lived tokens deleted.** The migration clears the two columns; dropping them is Task e1f359.


## Concept review (November 5, 2026, headless, after merging #1831)
Busy-Session moves collided with the harness rule (every hook best-effort). Built: the Stop hook peeks `pending_session_move` through `open_for_hook` (gap on failure) and, only when a move waits, spawns a setsid `lf __context-block --moment move`; the applier waits for its hook to exit, runs `new_thread`, then clears the row, so a killed apply retries at the next Stop. Live killed-mid-apply Stop is demo's.

## Concept review with Jack (November 5–6, 2026; page `scratch/concept-review.html`)
Accepted (Jack Heart, November 6, 2026), supersedes the matching parts of items 4 and 6; the options and proposal text are in git at 09923c9db8:
- **Switch at every launch (1b, "prefer more aggressive balancing").** Each launch takes the top-ranked login with room for its model, whichever is active.
- **Rank by use-it-or-lose-it ("account that resets soonest though should be balanced somewhat with remaining quota").** Remaining percent ÷ hours until reset on the launch's weekly window (model-scoped when it applies); the 5-hour window decides room only. Unscored logins rank after scored ones; plan first, declared order breaks ties.
- **Keep repository and Wave account orders (2b).** Accepted cost: disagreeing scopes switch the Machine's login back and forth.
- **Delete `lf session move --account` (3a) and `lf account <provider> drain`.** Keeping a login out (now `lf account pause`) switches away when that login is active; `lf account use` is the manual switch.
Unverified: whether a different account misses Claude's prompt cache for a moved conversation, making each switch cost quota.

Built (Nov 6): `route_rank`/`urgency`/`order_accounts_by_urgency`; the active-first reorder and the drain and `--account` move paths deleted; `SWITCH_SETTLE_SECS` 120 → 15 (questions 17); `provider_account::replacement_login` backs explicit-only switch-away (questions 18).

## Demo answer (Jack Heart, November 6, 2026)
Ranking fix accepted: "ok i guess. or just poll if necessary?" A window past its reset, and a login with no observed weekly window, score as a full week (100% over 168 hours) instead of ranking last. Built in the CLI pass (item 1 below).

## `lf account` CLI review (November 6, 2026; page `scratch/concept-review.html`)
Jack Heart asked for the full account API review, aiming for an excellent CLI, with the website mock as the floor (one row per login, a bar, percent left), LOO-593's API shape (`account.*`, `lf account`, `ops` dissolves, `lf auth` retired) and comps: teamclaude, ccdaddy, claude-swap, claude-rotate, maxpool, codex-acct, Paseo, HumanLayer. The Reddit post could not be fetched.
Proposed, not accepted: one row per login with problems as fix-it lines; verbs before logins (`lf account use work@`, provider only on ambiguity); `route` becomes `next`, a dry run with reasons; `set --routing explicit-only` becomes `pause`/`unpause`; `use` holds until full or `use auto`; `?` for unknown, never 0; reading age; reads render from cache and refresh stale rows only; exit codes.
Evidence checked Nov 6: Sonnet/Haiku 5.5 thinking blocks stay with the account that produced them (preserved-thinking docs); prompt-cache loss on a switch still unverified. Claude 2.1.319 has `.oauth_refresh.lock` and `CLAUDE_SECURESTORAGE_CONFIG_DIR`; lf now holds it on every write of the Claude login (item 1 below). The credential-only variable's behavior is unprobed.
Anthropic's legal and compliance page: developers "may not collect, store, or intermediate Claude.ai credentials or session tokens", and Pro/Max/Ultra limits "assume ordinary, individual usage". lf stores logins and sends access tokens to rentals. Unresolved; for Jack.

Answers (Jack Heart, November 6, 2026), verbatim: "1a for nwo", "2 a -- i think the reason for this is to have a single shared history and claude/codex resume worK", "3)a", "4)a", "5) not sure".
- Switch at every launch stays (1a), for now; no throttle or margin.
- No per-worker credentials (2a): one shared history so `claude --resume`/`codex resume` work from any terminal. `CLAUDE_SECURESTORAGE_CONFIG_DIR` is not used.
- `lf account use <login>` holds until that login fills up or `lf account use auto` (3a).
- All of it lands in #1828 (4a).
- The mock on the page is the build target, unconfirmed (5: "not sure"); show it at demo.

Not in this pass: `lf account --machines`, the fleet forecast line.
Built (Nov 6, implement), choices in `scratch/questions.md` 19–26:
1. `activation::ClaudeRefreshLock`: 2.1.319 takes proper-lockfile `mkdir <home>/.oauth_refresh.lock` then legacy `<realpath(home)>.lock` (stale 60s, touched every 5s, plus an optional `.oauth_refresh.lock.owner` record lf does not write). `activate`, `observe_active_account`, `renew_claude_login` and `subscription::fresh_claude_token` hold both around save-back/install/refresh write-back, touching every 5s, waiting up to 65s, taking over a stale one. `urgency` always scores (full week = 595); `read_first_usage` polls once after connect and import.
2. `provider_account::hold` (file per provider); `select_route` takes the held login when it has room, ends the hold when it is full.
3. Clap: `AccountCommand::{Next, Use, Pause, Unpause, Connect, Disconnect, Set, RedeemReset}` with one `target`/`login` string (`Target::parse`, `profile::resolve_login`); `Route`, `--details`, `--routing`, `provider` positional and `profile::show_routes`/`find_provider_account`/`tightest_window` deleted. Table: `account_status::render_table` with `Refresh::Stale`; `next`: `profile::show_next` (`tests/fixtures/dto/account_next.json` replaces `auth_routes.json`). Exit codes via `CommandExit`. Docs: subscriptions, ops, sessions, config, init skill.

## Demo answers, second pass (Jack Heart, November 6, 2026)
- Resume naming another login (questions 16): "b. When the named login differs from the one the running Codex process uses, restart it on the new login, as before, and say so in terminal 1. I guess". Built November 7.

Outstanding demo questions (bare login names, `pause` with no "disabled", Anthropic's terms, overage, open-ended waits), Jack Heart, November 7: "none of this concersn me". They stay as built.

## Built: November 7 pass
Merge 0f71d266c8 (#1831) undid main's `native_codex.py` and `session_launch_contract.rs`; both restored. `support`'s ctor moves `HOME` into the temp directory for every test binary (questions 27).

## Demo (November 7, 2026; store copy, temp HOME, network blocked)
Shown: `lf account --cached` rows match the mock; `next` gives reasons; ambiguity and unknown-login errors; the repository-order fix-it line. Not shown: `use`, `pause`, the refresh lock (they switch the real login). Jack Heart, on the three fixes: "go for it". Built, same day:
1. `lf account claude use jack@` answers with the current spelling, `lf account use claude=jack@` (any verb; `--cached`/`--json` before a verb keep their own line).
2. `lf account <login>` shows that login alone: auth, one `plan:` line (set and observed), each window with age and source, and `browser N:` lines for the profiles bound to it with whether this Chrome has them signed in; `--json` carries the same scoped `browser`. The provider form keeps the sections and every saved browser. The `source=managed_home configured plan=… expires=…` footer is gone from both.
3. `lf account use bogus@` (unknown or ambiguous) prints the resolver's message and exits 2, Clap's usage code; documented beside the 2/3 contract.
Demo hazard: `lf account use` with a temp HOME still reads stored logins from the real `~/.lf/accounts/` (absolute homes in the store) and calls the real `security`; it failed before writing. Demos of switching need fake logins in a fake LF_HOME, not a store copy.

## Five whys: this Task's own run stopped on November 7, 2026 (live, installed lf e3c888924)
Jack Heart: "also put the 5hwys fixes in tis diff".
Seen: compress on bare `-a claude` (default model Fable) ended with claude_result "You're out of usage credits. Switch to another model, or manage usage credits at https://claude.ai/settings/usage?from=cc_cli_limit_message, to continue."; recovery failed with "execution failed: AgentProcess retry must retain its thread unless the selected account changes"; two automatic restarts then failed at once with "no eligible managed claude account: 'jack@loopflow.studio' cooling until 2026-11-10T06:00:05Z, 'jackstah@gmail.com' cooling until 2026-11-11T12:00:05Z" / "provider_rate_limited: rejected: seven_day_overage_included". Store at the time: jack@ (`loopflow`) weekly 93% used, weekly:fable 88%; account-wide `cooldown_until` 1794290405 reason "rejected: seven_day_overage_included" (cleared by hand Nov 7 to unblock). jackstah@ (`primary`) weekly:fable 100% until 1794398400, weekly 93%; its login also reads "needs login (identity or credential rejected)", cause unknown.
Whys: an overage rejection named no model and became an account-wide cooldown to the weekly reset; the bare-harness model fallback hit main's retry invariant; `lf task run`'s attempts relaunched straight into "no eligible account"; only SQL cleared the wrong cooldown.
Fixes 1–4, built November 7:
1. `RateLimitSignal.credits` / `SpentLimit.credits` (overage types; "out of usage credits"). `record_rate_limit_signal` scopes a credits limit to the route's launched model (`ProviderAccountRoute::launching`), else `CLAUDE_DEFAULT_MODEL_FAMILY`; no account cooldown. Test: `a_credits_rejection_holds_the_launched_model_and_never_the_account`.
2. The branch's invariant already ignores model; `session_record::retry_keeps_its_thread` names it. `a_credits_limit_goes_on_with_another_model_in_the_same_thread` runs the live sequence through the step loop.
3. Steps already wait on a known reset (`CapacityWait`). Restarts no longer start into one: `restart_ships` skips a run whose `waiting_on_limit` reset is ahead, and `run_task_graph_for` stops relaunching such a run and leaves it to the sweep.
4. A fresh reading refutes a rejection's cooldown wherever one is saved: `provider_account::save_polled_usage` is the only `"poll"` write (`lf account` refresh, `record_stated_limit`, connect, Codex reset). It clears the account cooldown when the reading has account-wide windows and none is spent; a reading with none says nothing. Tests: `a_fresh_reading_with_room_clears_a_cooldown_a_rejection_set`, `a_stated_limit_whose_poll_reads_room_leaves_the_account_routable`.
5. Two Keychain items for one service (this Mac: acct "unknown", 2026-10-18; acct "jack", 2026-11-06) failed a Nov 7 switch with "the item did not take the login". Claude 2.1.319 reads `security find-generic-password -a "$USER" -w -s <service>` (`USER` if it matches `^[a-zA-Z0-9._-]+$`, else `claude-code-user`). `claude_keychain_account` names that; reads and the write's check go by it, and the write also updates the first other item `-s` finds. Test: `a_keychain_write_lands_on_the_item_claude_reads_when_a_service_holds_two` (fake `security`).

## Remaining
- A third Keychain item for one service stays stale (only the first other item is rewritten); Claude reads by account, so it is unread. Fix 5 is unshown on the real Keychain: demo with a fake login.
- Concept review after fix 5 (headless, November 7), for verify: `record_stated_limit` now clears the cooldown its own no-model spent line just set whenever the poll right after reads every account-wide window under 100%. If Claude keeps saying "You've hit your limit" while its usage endpoint reads room (a window the endpoint does not report), each launch lands on the same login, fails, and clears again; the step loop's attempt budget is the only bound. Unseen; the live reading behind fix 4 was a credits line, which no longer sets an account cooldown at all. Check at demo whether a no-model spent line ever coincides with a poll showing room.
- Credits limits share one window key (`seven_day_overage_included`), so a credits rejection on a second model moves the hold to it. Only Fable needs credits today.
- `ops::task::tests::{nonresumable_execution_blocker_remains_a_launch_refusal, a_dead_graph_is_history_while_a_live_graph_process_retains_the_checkout, task_workspace_context_covers_committed_staged_unstaged_and_untracked_bytes}` fail here with "claiming needs your Git user.email" even with a global identity in the test HOME; not reached by this pass's diff, not checked on origin/main.
- Single-pass review, November 7: the old-spelling correction treats any leading target as a provider. `lf account alice@ use bob@` suggests `lf account use alice@=bob@`, which cannot resolve as a provider. Keep for verify; the intended `lf account claude use bob@` correction works.
- Scheduled renewal is unexercised against a live rental; demo owns it (`lf machine credentials renew` with one live rental, then none). `--owned` is likewise unexercised on a real rental: it takes a browser approval, so tests cover it with a stand-in `ssh`.
- Whether a Claude refresh cancels earlier access tokens is unverified (needs a live refresh of a real login); the resend-to-every-rental after each refresh is correct either way.
- Removal Task e1f359 (`isolate:`, long-lived token columns, `provider_session_accounts.isolated`) exists, unstarted; its Linear export is unconfirmed.

Still true from the first pass:
- A step restarted after dying mid-wait runs in a new Run: checkout and completed steps kept, the step's own conversation not (LOO-435 owns resuming it).
- `limit_recovery::waiting_on_limit` names the step by label, so a skill repeated in a Graph restarts at its first occurrence.
- Native Claude limit handling (items 12–14) comes from reading Claude's 2.1.319 source, not a live limit. Codex terminal limits are seen only through the relay's stream.
- No draft signal, so `lf session move` moves every available Session (questions 12). Desktop's Blocked filter waits for a Sessions list surface.
- Demo: the demo-decision build (scoped limits, fall-through, `session move`, urgency routing, `pause` switch-away), a hold beating an order, and `lf resume --account` restarting a running Codex or switching a running Claude's login have not been shown on the configured path; no paid quota may be spent.
- Open at demo: the mock as build target (answer 5).
- Unverified, sharper now that launches switch often: a running Codex keeps its own login in memory; if it refreshes and writes `auth.json` after the home switched to another account, the home silently flips back and the other account's rotated refresh token, if it rotated in between, is lost (`save_back` runs only at the next switch). Demo checks: after a switch, does a running Codex's refresh rewrite `~/.codex/auth.json`?

## Concept reviews (November 7, 2026, headless), all built
1. **`lf account use` beats scoped orders** (questions 30). `held_login` runs before the order in `select_route`; `inspect_provider_route` puts the held login first, so `next`, pause's replacement and the rental pick agree. Naming the held login counts as the hold. Counterexample, accepted as reversible: a repository order keeping a client's work on its login is overridden while a hold stands.
2. **`lf --account claude=b@ resume <running Claude>` switches, then attaches** (questions 28).
3. **Old spelling fails as unknown;** `lf ops account` is the only command with a positional beside subcommands.
4. **`--blocked` and `--on` read the login the Session uses:** a running Codex its recorded account, everything else the shared home.
5. **`--on` takes a login as every command does,** through `account::resolve_on`.
6. **A restarted wait keeps its `-a`.** `agent_limit_recoveries.agent` records the step's override; `ship::restart_lf_in` passes `--agent` and clears the reader's `LF_AGENT_OVERRIDE`/`LF_SETTINGS`. A ship process that dies outside a wait restarts with no `-a`: no wait row holds it.
7. **An owned rental.** `lf machine connect <rental> claude <email> --owned` takes `fresh_machine_login`, judged by `inspect --full`; `install_managed` replaces an access token with the full login. Renewal leaves a rental holding a full login alone, and asks the rental before skipping an account the laptop holds no rental login for.

Proposed for Wave memory: a restarted wait keeps its recorded `-a`; `--owned` gives one rental its own Claude login, which renewal leaves alone; `StopFailure` recording runs in its own OS session.

## Concept review (November 7, 2026, headless, after items 6–7), built
8. **A dead parked login is out of rotation before a switch.** `identity::check_routable_identity` refreshes a Connected parked login whose token is due (`NeedsLogin` → `Missing`, `Unavailable` skips this launch), holding the native refresh lock; the active login is never refreshed there.
9. **Fix-it text parses.** Every printed `lf account connect|disconnect|set|use` is `provider=login`. `account_commands_printed_by_errors_parse` scans the sources that print them and runs each through `normalize_args` and Clap. `docs/subscriptions.md` corrected.

## Concept review (November 7, 2026, headless, after item 9), built
10. **The limit hook cannot be killed mid-refresh.** `--moment limit` starts `--moment record-limit` detached (`os_process::detached_context_block`) and exits 0; failures note a `RecordingGap`. Accepted cost: a message sent before the recorder finishes can fail once on the old login. Still lost: a limit when the active login matches no stored copy and Claude's refresh lock outlasts its 65s wait.
11. **The deleted spellings left the tests;** `--shared-provider-home` is the fixture's own flag.

Gate/CI: `claude_shared_home.py` (writes a Keychain item), `codex_connect.py --shared-provider-home`, `remote_file_observer_keeps_the_recorded_owner_and_streams_changes`.

## Concept review (November 7, 2026, headless, after #1842), built
12. **Only spent quota moves the login, in terminals as in Graph steps.** `StopFailure`'s `rate_limit` covers every 429 (input fields read in the 2.1.319 source, not seen live). The hook records only when `agent::subscription_limit_text` reads `last_assistant_message` as spent: "You've hit your", "You've reached your", "You're out of usage credits" (Claude's `xAo` list), not `Fable 5 requires usage credits` or seat-type lines.

## Concept review (November 7, 2026, headless, after item 12), built
13. **A spent line naming a model scopes its limit.** `subscription_limit_text` returns `SpentLimit { reason, model }` (`opus|sonnet|haiku|fable` between "your" and "limit"); `stated` carries it. The terminal recorder still moves the login: a no-model `select_route` is held by every scope.
14. **A spent account stays out until its window resets.** `record_stated_limit` (native recorder, before the switch; `record_limit_blocking` for steps) polls a Claude account once. `subscription::poll_account` gives every 100% window `limited_until` at its reset, so `lf account` polls no longer clear a stream-set hold. A failed poll keeps 15 minutes. Codex is not polled (questions.md). Test builds default the usage URL to `127.0.0.1:1`.

## Concept review (November 7, 2026, headless, after item 14), built
15. **A capacity 429 retries, in steps as in terminals.** Claude's `Request rejected (429) · this may be a temporary capacity issue` classified as nothing and failed the Graph Blocked; "capacity issue" now reads as `AgentFailure::Capacity` (43c2d6a69). The open doubt here, whether "You're out of usage credits" is the account's line or Fable's, was settled live on November 7: Fable's (five whys, fix 1).

Check, November 7 at 89dcf67b7 (empty cwd, temp HOME, LF_* unset, stdin closed): lib `provider_account::`, `provider_auth::`, `subscription::` 164 pass. Live spent limits: demo.

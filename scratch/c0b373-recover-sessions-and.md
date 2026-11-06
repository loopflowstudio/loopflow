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
7. **No model/harness fallback.** Preferences have one resolved agent and no ordered alternates, so a fallback would be invented policy. Accounts within the existing route only. Surface at review. Superseded at demo (Accepted at demo, item 3).
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


## Concept review (November 5, 2026, headless)
The concepts hold: an account cools, a step goes on under another or waits, and status reads Running, Waiting or Blocked. Fixes built: the restart contract names its one exception (a step that ended waiting on a usage limit); `restarts_by_itself` decides both the sweep and whether status promises an automatic restart.

## Accepted at demo (Jack Heart, November 5, 2026)
Supersedes decision 7 and answers `scratch/questions.md` 1–2; the reasoning and evidence are there. Build all of this in PR #1828, with no stacked Task.

1. **Login switch reaches open Claude Sessions.** Live test: a running Claude 2.1.318 re-reads the Keychain only when `<config>/.credentials.json` changes mtime or is absent. `activate` updates that file's mtime (when present) after writing the Keychain. Jack's stale stub in `~/.claude` was removed. Decision 6 then holds; keep it for shared Sessions only. Isolated Sessions never move by themselves.
2. **Scoped limits.** A limit records its scope (all models or one model, e.g. Fable). Cooldown, room ranking and blocked-ness apply only to launches that scope affects; a Fable limit leaves Opus/Sonnet on that account usable. Stream decoding keeps the provider's scope rather than collapsing to `weekly`/`session`; unknown limit types become their own window, never a fixed list.
3. **Fall-through for headless steps.** Order: another account, then another model, then another provider, then wait. An explicit `harness:model` stays on that model (accounts only). Bare `claude` or `codex` may also change model within that provider. Only preference-resolved agents (no explicit agent) may change provider, within an ordered preference list. A provider change continues through the existing history import (`lf resume -a` path), not a fresh start.
4. **Bulk moves for interactive Sessions.** One typed Session selector, spelled as flags, shared by `lf session list` (the preview) and actions: `--blocked` (the Session's next message would fail, derived from its recorded account/model plus scoped limits) and `--on <account|harness[:model]>`. `lf session move <selector> --account …|-a …` moves the selected Sessions; it requires a selector. `lf account drain <account> [--to …]` stops routing there and moves what's on it gently (idle now, busy after the turn, a draft-holding Session listed as skipped). Interactive Sessions only; the command filters explicitly. Moves never write preferences. Desktop gets the same actions over the same selector (Linear-style filtered view, select, act). A query string is not planned.
5. **No provider pins in Loopflow.** Before landing, nothing built-in or in this repo's `.lf/config.yaml` requires Claude or Codex; remove the five `skills.*.agent` pins (including `xor-route: claude:sonnet`; Jack sets cheaper models as personal preferences if wanted).
6. **Route by soonest reset (Jack, November 5).** Among eligible accounts with room for the launch's scope, prefer the one whose limiting window resets soonest, so no account resets with quota unused. This replaces "most room" ordering (`room_rank` / `order_accounts_by_room`). Explicit route order still decides among accounts outside automatic routing; ties fall back to most room, then declared order.

## Build plan for the demo decisions (November 5, 2026)
Owners, one each. Routine choices are recorded in `scratch/questions.md` 6–10.
- **Login switch**: `activation::write_login` touches `<native>/.credentials.json` after a Claude write, when present.
- **Scoped limits**: `RateLimitSignal.model` (None = every model). Windows keep the provider's `rate_limit_type` verbatim; a `*_opus|sonnet|haiku|fable` suffix is that model's scope. Account-wide limits still cool `provider_accounts.cooldown_until`; a model-scoped limit cools only its window row (`provider_account_limits.model`, `limited_until`, added in this Task's migration). `account_route_eligible` and `tightest_window` take the launch model; a launch with no model is held back by every scope.
- **Fall-through**: `AgentConfig.fallback_agents` carries the rest of a `default_agent` list (`lf set default_agent claude,codex`) only when no explicit agent applies. `_run_with_transient_retries` climbs a ladder on `NoEligibleAccount` with a reset: same agent other model (bare harness only; Claude `opus, sonnet, haiku`), then the next preference agent (history import through `conversation_log::continuation`), then waits. An explicit `harness:model` has no rungs.
- **Selector**: `SessionFilter.blocked` / `.on`; `SessionRecord.blocked` (DTO, fixtures in both languages). Blocked = the Session's account (pinned, else the provider's active login) cannot take its model now.
- **Moves**: `lf session move (--blocked|--on <x>) (--account <email>|-a <agent>)` and `lf account <provider> drain <email> [--to <email>]`, both in `ops::human_session::moves_bulk`. Shared same-provider moves switch the login once; a Session pinned to its own login is listed with the `lf resume --account` that moves it (questions 13); an agent or model change goes through `new_thread`, a seeded thread (questions 14). Busy Sessions move at their next Stop hook (`session_moves`, applied by `__context-block stop`); no draft signal exists yet (questions 12). Drain sets the account `explicit-only` and moves to `--to` or the route's next login.
- **Soonest reset**: `order_accounts_by_reset` / `route_rank` rank plan, then the limiting window's reset, then room (questions 11).
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
- **Isolation deleted.** `RouteHome` is gone: a stored login (`AccountLogin::Stored`) always launches in the shared home; only replay (`AccountLogin::Replayed`) runs in a recorded capture's profile, so this Machine's login never switches to another catalog's account. `--isolate`/`--shared`, `LF_ACCOUNT_ISOLATION`, `launch_isolated`, `prepare_isolated_codex`, `provider_session_isolated`, `names_another_account`, `Departure::Account` and the restart-based move are deleted. `isolate:` stays in the catalog as Inactive (read, ignored) until Task e1f359.
- **Legacy isolated conversations.** `provider_session_accounts.isolated = 1` now means only "its transcript is still in that account's home". `adopt` copies it into the shared home on resume; `pin_provider_session_route` always writes `isolated = 0`, so readers (`transcript::conversation_home`) switch to the shared home once it ran there. `lf connect` of a conversation found in an account home copies it first. A resume never names the account it began under (`provider_session_routed` drops the exact account).
- **Sessions follow the shared login.** `LaunchCapacity::login/blocks` read only the active login; `lf session move --account` no longer skips "pinned" Sessions.
- **Remote logins.** Owned Machine: `lf machine connect <m> claude <email>` sends a fresh full login (`fresh_machine_login`, as Codex), installed through `install_managed`. Rental (`rental_for_machine`): the laptop keeps one Claude login per account under `~/.lf/accounts/rental-logins/claude/<id>/`, refreshes it through `subscription::claude_access_token` when within `ACCESS_RENEW_SECS` (2h) of expiry, and sends only `Credential::ClaudeAccess`; after a refresh it resends to every live rental. The rental replaces an older access token and, when its shared home was on it, that too (`activation::renew_claude_login`, mtime marker). `installed` reports an access token stale within 2h, so any connect or launch prefix renews it. `lf --machine <m> account` shows `access token until <time>` (`expires_at`).
- **Long-lived tokens deleted.** `lf account token`, `mint_long_lived_token`, `claude setup-token` parsing and its PTY auth path, `LongLivedToken` and the store accessors. The migration clears the two columns; dropping them is Task e1f359.


## Concept review (November 5, 2026, headless, after merging #1831)
Busy-Session moves collided with the harness rule (every hook best-effort). Built: the Stop hook peeks `pending_session_move` through `open_for_hook` (gap on failure) and, only when a move waits, spawns a setsid `lf __context-block --moment move`; the applier waits for its hook to exit, runs `new_thread`, then clears the row, so a killed apply retries at the next Stop. Live killed-mid-apply Stop is demo's.

## Concept review with Jack (November 5–6, 2026; page `scratch/concept-review.html`)
Accepted (Jack Heart, November 6, 2026), supersedes the matching parts of items 4 and 6; the options and proposal text are in git at 09923c9db8:
- **Switch at every launch (1b, "prefer more aggressive balancing").** Each launch takes the top-ranked login with room for its model, whichever is active.
- **Rank by use-it-or-lose-it ("account that resets soonest though should be balanced somewhat with remaining quota").** Remaining percent ÷ hours until reset on the launch's weekly window (model-scoped when it applies); the 5-hour window decides room only. Unscored logins rank after scored ones; plan first, declared order breaks ties.
- **Keep repository and Wave account orders (2b).** Accepted cost: disagreeing scopes switch the Machine's login back and forth.
- **Delete `lf session move --account` (3a) and `lf account <provider> drain`.** `lf account set <p> <login> --routing explicit-only` switches away when that login is active; `lf account use` is the manual switch.
Unverified: whether a different account misses Claude's prompt cache for a moved conversation, making each switch cost quota.

Built (Nov 6, implement): `route_rank`/`urgency`/`order_accounts_by_urgency`; the active-first reorder (`select_shared_account`, `active_candidate`) deleted; `SWITCH_SETTLE_SECS` 120 → 15 (questions 17); `MoveTarget`, `follow_login`, `switch_login`, `drain_account`, `AccountCommand::Drain` and `Write::SessionMove.account` deleted (Swift `sessionMove` too); `provider_account::replacement_login` backs explicit-only switch-away (questions 18); docs updated.

## Demo answer (Jack Heart, November 6, 2026)
Ranking fix accepted: "ok i guess. or just poll if necessary?" A window past its reset, and a login with no observed weekly window, score as a full week (100% over 168 hours) instead of ranking last. Not built.

## `lf account` CLI review (November 6, 2026; page `scratch/concept-review.html`)
Jack Heart asked for the full account API review, aiming for an excellent CLI, with the website mock as the floor (one row per login, a bar, percent left), LOO-593's API shape (`account.*`, `lf account`, `ops` dissolves, `lf auth` retired) and comps: teamclaude, ccdaddy, claude-swap, claude-rotate, maxpool, codex-acct, Paseo, HumanLayer. The Reddit post could not be fetched.
Proposed, not accepted: one row per login with problems as fix-it lines; verbs before logins (`lf account use work@`, provider only on ambiguity); `route` becomes `next`, a dry run with reasons; `set --routing explicit-only` becomes `pause`/`unpause`; `use` holds until full or `use auto`; `?` for unknown, never 0; reading age; reads render from cache and refresh stale rows only; exit codes.
Evidence checked Nov 6: Sonnet/Haiku 5.5 thinking blocks stay with the account that produced them (preserved-thinking docs); prompt-cache loss on a switch still unverified. Claude 2.1.319 has `.oauth_refresh.lock` and `CLAUDE_SECURESTORAGE_CONFIG_DIR`; `rust/loopflow/src` takes no such lock, so a Claude refresh can land during a swap. The credential-only variable's behavior is unprobed.
Anthropic's legal and compliance page: developers "may not collect, store, or intermediate Claude.ai credentials or session tokens", and Pro/Max/Ultra limits "assume ordinary, individual usage". lf stores logins and sends access tokens to rentals. Unresolved; for Jack.

Answers (Jack Heart, November 6, 2026), verbatim: "1a for nwo", "2 a -- i think the reason for this is to have a single shared history and claude/codex resume worK", "3)a", "4)a", "5) not sure".
- Switch at every launch stays (1a), for now; no throttle or margin.
- No per-worker credentials (2a): one shared history so `claude --resume`/`codex resume` work from any terminal. `CLAUDE_SECURESTORAGE_CONFIG_DIR` is not used.
- `lf account use <login>` holds until that login fills up or `lf account use auto` (3a). Supersedes the "use lasts until the next launch" item in the headless concept review below.
- All of it lands in #1828 (4a).
- The mock on the page is the build target, unconfirmed (5: "not sure"); show it at demo.

Next implement pass builds, in order:
1. Correctness: hold Claude's `.oauth_refresh.lock` (read its protocol in the 2.1.319 binary first) across `activate`'s save-back and install; full-week score (100% over 168h) for a window past its reset and a login with no weekly reading; one usage poll at `lf account connect`.
2. `use` hold: a stored per-provider hold that launches honor until the held login has no room or `use auto` clears it.
3. CLI as on the page: `lf account` one row per login (bar = weekly window, percent left, reset, `● active`, reading age when stale, `?` for unknown), cache-first with stale rows refreshed; fix-it lines with the exact command; `lf account <login>` detail (replaces `--details`); `next` (replaces `route`) with reasons; `pause`/`unpause` (replaces `set --routing explicit-only`, still switching away when active); verbs first with `claude=a@` only on ambiguity, the same spelling in `--account` and `connect`; exit codes 0 fine, 2 blocked, 3 login needed on `next` and `use`. Old spellings deleted, not aliased. Docs follow.
Not in this pass: `lf account --machines`, the fleet forecast line.
Built (Nov 6, implement), choices in `scratch/questions.md` 19–26:
1. `activation::ClaudeRefreshLock`: 2.1.319 takes proper-lockfile `mkdir <home>/.oauth_refresh.lock` then legacy `<realpath(home)>.lock` (stale 60s, touched every 5s, plus an optional `.oauth_refresh.lock.owner` record lf does not write). `activate`, `observe_active_account` and `renew_claude_login` hold both around save-back/install, touching every 5s, waiting up to 65s, taking over a stale one. `urgency` always scores (full week = 595); `read_first_usage` polls once after connect and import.
2. `provider_account::hold` (file per provider); `select_route` takes the held login when it has room, ends the hold when it is full.
3. Clap: `AccountCommand::{Next, Use, Pause, Unpause, Connect, Disconnect, Set, RedeemReset}` with one `target`/`login` string (`Target::parse`, `profile::resolve_login`); `Route`, `--details`, `--routing`, `provider` positional and `profile::show_routes`/`find_provider_account`/`tightest_window` deleted. Table: `account_status::render_table` with `Refresh::Stale`; `next`: `profile::show_next` (`tests/fixtures/dto/account_next.json` replaces `auth_routes.json`). Exit codes via `CommandExit`. Docs: subscriptions, ops, sessions, config, init skill.

## Remaining
- Scheduled renewal is unexercised against a live rental; demo owns it (`lf machine credentials renew` with one live rental, then none).
- Whether a Claude refresh cancels earlier access tokens is unverified (needs a live refresh of a real login); the resend-to-every-rental after each refresh is correct either way.
- Laptop-side status per Machine is `lf --machine <m> account`; a single laptop view across Machines is not built.
- Removal Task e1f359 (`isolate:`, long-lived token columns, `provider_session_accounts.isolated`) exists locally; its Linear export was rate-limited on November 5 and still awaits confirmation.

Still true from the first pass:
- A step restarted after dying mid-wait runs in a new Run: checkout and completed steps kept, the step's own conversation not (LOO-435 owns resuming it).
- `limit_recovery::waiting_on_limit` names the step by label, so a skill repeated in a Graph restarts at its first occurrence.
- Native Claude limits cool for the 15-minute default: `StopFailure` carries no reset time, and it was confirmed from Claude 2.1.318's schema, not a live limit. Codex terminal limits are seen only through the relay's stream.
- No draft signal, so `lf session move` moves every available Session (questions 12). Desktop's Blocked filter waits for a Sessions list surface.
- Demo: the demo-decision build (scoped limits, fall-through, `session move`, urgency routing, explicit-only switch-away) has not been shown on the configured path; no paid quota may be spent.
- Open for Jack at demo: `scratch/questions.md` 3, 4, 11, 16, 17, 18.
- Unverified, sharper now that launches switch often: a running Codex keeps its own login in memory; if it refreshes and writes `auth.json` after the home switched to another account, the home silently flips back and the other account's rotated refresh token, if it rotated in between, is lost (`save_back` runs only at the next switch). Demo checks: after a switch, does a running Codex's refresh rewrite `~/.codex/auth.json`?

## Concept review (November 6, 2026, headless, after urgency routing)
The concepts hold: one login per provider per Machine, the top-ranked login at every launch, explicit-only to keep a login out. What the accepted decisions imply, for implement:
- **`lf account use` lasts until the next launch.** With switch-at-every-launch, the next launch moves the login back when another ranks ahead. Docs say so where `use` appears (`docs/subscriptions.md` "Switch the shared account", `docs/sessions.md`, `docs/ops.md`): to stay on one login, name it (`--account`) or set the others `explicit-only`.
- **Stale ranking text.** `docs/ops.md:314` still says "Routes try accounts in order; one at 95% of a usage window drops behind the rest"; it becomes the urgency rule. The test comment at `provider_account.rs:3841` ("resets soonest") names the observed-before-unobserved rule instead.
Open: none new; questions 3, 4, 11, 16, 17, 18 stay for demo.
Built (Nov 6): both items; the `use` part is superseded by the hold (3a below).

Checks (Nov 6, after compress: logins resolve once per verb, `Target::parse_login`, one `tightest_weekly`): lib `provider_account:: lf::commands::account lf::commands::profile lf::tests::account` (93, 1 ignored) and integration `auth_tests:: cli_discovery:: documented_commands::` (47) pass as built binaries from an empty dir with isolated `HOME`; fmt, clippy clean. Gate: `served_reads` (needs a canonical `TMPDIR`), Swift in CI.

# Open questions for review (c0b373)

Assumptions made in the headless implement step, each reversible.

1. **Interactive continuation.** The login moves by itself; the interrupted turn continues on the person's next message. Nothing types into a terminal, to keep active input safe. Automatic continuation of idle Sessions would need a reliable "no draft in progress" signal. Answered at demo: shared Sessions follow the login (the `.credentials.json` mtime fix); everything else moves through `lf session move -a`. `lf account drain` and `session move --account` are deleted (Jack, November 6).
2. **No model or harness fallback.** Preferences hold one resolved agent with no ordered alternates, so any fallback would invent policy. A model-scoped limit cools the whole account. Overturned at demo: limits are scoped and headless steps fall through account, model, provider (Accepted at demo, items 2–3).
3. **A rejected event cools the account until its reset**, including `seven_day_overage_included`. If overage rejection leaves the plan's own quota usable, this cools too much.
4. **Waits have no upper bound.** A weekly limit can hold a step for days, in one idle process, until someone interrupts it.

# Jack's direction at demo (November 5, 2026; under discussion, not yet accepted design)

Overturns decision 7 and question 2 as policy. Exact shape still open.

- Fail through in order: another account, then another provider (and model), and only when everything explicitly allowed is exhausted, fail or wait and retry. Tasks can declare, or be assumed by default, that they may switch between Claude, Codex and OpenCode.
- Jack is unsure of the order: perhaps account, model, provider; perhaps provider and model combine; perhaps different priority orders.
- Limits come in many kinds (5-hour windows, which may be gone; Claude's Fable-specific limit). Detect them all, and ideally anticipate them.
- Load balance Task work across as many accounts as possible instead of filling one and moving on. Open idea: Task workers balance across accounts while interactive Sessions share a single account, which keeps switching them simple.

Scope (Jack, November 5): this discussion covers Claude and Codex only. OpenCode and Pi accounts generalize later in their own Task.

# Account separation research (November 5, 2026; from code and live state, no login touched)

- Live: all 34 running Claude processes use `~/.claude`, none isolated. Recorded conversations: Claude 5,680 shared / 261 isolated; Codex 5,728 shared / 3,317 isolated. `isolate:` and `--isolate` apply to the launch and its children; nothing distinguishes Sessions from Runs.
- Decision 6 and `running_agents_follow_native_login` are unproven and likely wrong as "immediately". Claude 2.1.318's minified change check stats `.credentials.json` mtime; while the cached token is usable it does not re-read the Keychain. lf's switch (`write_claude_keychain_credential`) writes only the Keychain, and `~/.claude/.credentials.json` is dated October 18 despite 46 logged switches. A 401 or expiry does re-read; a usage limit is neither. A `X()`-gated versioned path exists whose state on this Machine is unverified. Needs a live probe.
- `used_account` credits usage to the newest switch on the same assumption, so attribution may be wrong after a switch.
- Switch log disagrees with the login: last row moves to `loopflow` at 15:30 UTC, `lf account route` reports `jackstah@gmail.com` at 16:54. Unexplained.
- Cause is unreliable: 42 of 46 Claude and 700 of 732 Codex switches say `person`; a launch with a named account list records Person (likely, not confirmed).
- Split hazard: refresh tokens rotate, and the same login in both the native home and an isolated profile holds two copies. A dedicated split avoids this; a soft reserve does not, unless workers use a long-lived `lf account token`.

# Interactive policy (Jack, November 5, 2026)

Bias against an interactive Session changing provider by itself; no surprising experiences. Task workers do retry and move automatically, within their approved harnesses. Whether interactive Sessions move automatically at all is still open. Required either way: a command, and probably a Desktop button, that transfers all open Sessions in the least disruptive way. It must cover isolated and shared Sessions, and three targets: from Claude to Codex when Claude is out, to another Claude account, to Opus when Fable runs out.

Bulk move scope (Jack, November 5): default to blocked Sessions, not all. Interactive Sessions only, never Runs. Sketch under discussion: blocked means the Session's account or model has no capacity, derived from limit observations, so provider follows from what ran out; isolation decides how a Session moves, not whether. `--all` widens it. Gaps: no per-Session blocked state (`agent_limit_recoveries` is keyed by headless Process); account-wide cooldown cannot separate a Fable Session from a Sonnet one until limits are scoped; a shared Session's real account is uncertain until the live login test.

Session selection (Jack, November 5): a hidden "blocked only" default on move is confusing. Jack imagines a query over Sessions with actions on top; `all blocked` is the most important query. Sketch under discussion: `lf session list` filters are the query, the same selector flags go on every bulk action (`--blocked`, `--on <account|agent>`), and list is the preview. Replaces the earlier `move --all` sketch: `--all` already means every repository on `list`, and `--account` / `-a` are move targets, so filters need other names.

Comps chosen (Jack, November 5): kubectl and Linear. CLI follows kubectl: one selector shared by list and actions, list as preview. Desktop follows Linear: filtered view, select, act. Query string versus flags and `drain` remain undecided.

Selector (Jack, November 5): start with flags; simplicity wins. Implement one typed selector so a later `key:value` query string would be only another way to write it. Query language not planned.

Live login test (November 5, Claude 2.1.318, Jack approved): fake tokens in a throwaway Keychain item, local stand-in API recording token hashes, no real login or quota. TUI and stream-json agree. With `.credentials.json` present, a Keychain change was not picked up (still old token after 36s and 72s); updating that file's mtime moved the next request to the new token within 3s. With no such file, the Keychain change was picked up by the next turn after 36s.
`~/.claude/.credentials.json` on Jack's Machine is a stub: 22-character token, expiresAt 4102444800000, matching this repo's test fixtures, dated October 18. It likely leaked from a test and likely explains why `/login` does not spread. Removal is Jack's call.
Fix for #1828: touch that file after writing the Keychain in `activate`; decision 6 then holds.
Scope (Jack): the bulk move goes in #1828 (Jack said 1820, which is merged and unrelated), not a stacked Task.

Stub removed from `~/.claude` (Jack approved, November 5); `/login` spreading across real Sessions still to be seen in use.
Drain (Jack, November 5): `lf account drain` is accepted as the gentle account-first action; `lf session move` is the more aggressive Session-first one. Same implementation.

Worker fallback (Jack, November 5): an explicit agent is followed only within itself. `claude:<model>` stays on that model; bare `claude` may fall back across Claude models (e.g. Fable to Opus) and accounts. Preferences (default agent, no explicit agent) are where cross-provider fall-through lives. Before landing, nothing built-in or in this repo's config requires Claude or Codex explicitly; Jack only ever sets preferences.
Interactive automatic moves: start shared-only (Jack); trade-offs explained at review.

# Why isolation exists (research, November 5)
Jack's goal: use several Max accounts to spread work without thinking about which one is in use. Jack wants isolation settled in this Task (#1828).
- Before October 4 every launch ran in its account's own home: that was the load-balancing mechanism. #1416 (October 4) made the shared native home the default because conversations scattered across homes and plain `claude --resume` / `codex resume` could not find them. Isolation stayed for already-pinned conversations and explicit `--isolate`.
- Today only `lf ssh` sets isolation automatically (remote Machines). Locally, no isolated Claude conversation since October 27 and no Codex one since October 28; none of the running Claude processes are isolated. Shared: 5,312 Claude conversations since November 2.
- Weekly limits make sequential use as good as concurrent for total capacity; isolation's concurrency helps only short windows (5-hour) or throughput. Capacity is lost only when an account resets with room left, which suggests routing by soonest reset among accounts with room rather than most room.
- Gaps that kept shared mode from meeting the goal: the switch did not reach running Claude (stub file; mtime fix), and limits were misclassified or unhandled (this PR).

# Assumptions while building the demo decisions (November 5, 2026)

6. **Model scope names.** Claude 2.1.318 knows `five_hour`, `seven_day`, `seven_day_overage_included`, `seven_day_opus`, `seven_day_sonnet`, `seven_day_cowork`, `seven_day_omelette`, `seven_day_oauth_apps`. Only a suffix naming a model lf can launch (`opus`, `sonnet`, `haiku`, `fable`) scopes a limit; `seven_day_omelette` cools the whole account until its model is known.
7. **A launch with no model** (bare `claude`) is held back by every scoped limit on an account, since lf cannot tell which model Claude would pick. Model fall-through then names one.
8. **Ordered preferences** are a comma list in `default_agent` (`lf set default_agent claude,codex`), mirroring account routes. The first entry is the agent; the rest are providers a headless step may fall through to.
9. **Claude model rungs** are `opus`, `sonnet`, `haiku` in that order, skipping the model already running; Codex limits are account-wide, so it has no model rungs.
10. **Busy Sessions** move at their next Stop hook rather than mid-turn; a Session whose program status reports a draft is skipped and listed.
- Remote isolation (`lf ssh`, `f72bc3ab9d` #1493): set only when a launch selects accounts. Two reasons: a laptop-selected account must not switch the target's shared login under the target's own work, and SSH launches need file-backed credentials (the macOS Keychain is generally unusable from SSH). Jack doubts it is essential. Both look replaceable on Jack's own Machines: a shared home can hold a file login (Claude falls back to a plaintext `.credentials.json`; mtime change is what running Claude watches). That would make isolation deletable everywhere, pending Jack's choice.
- Comps for remote credentials (web research, secondary sources mostly): Paseo runs a daemon on the target with agent CLIs logged in there; nothing is forwarded. Omnara wraps the local Claude session; its own auth covers only its dashboard. Warp cloud agents use API keys stored as Warp secrets, billed to the API, not subscriptions. None forward subscription logins. Anthropic documents `CLAUDE_CODE_OAUTH_TOKEN` (setup-token) as the long-lived, inference-only route for headless machines; lf exposes it as `lf account token`. Forwarding a refreshing OAuth login creates a second copy that rotation can break, independent of isolation; a long-lived token avoids that.

# HumanLayer comp (github.com/humanlayer/humanlayer, read November 5)
- One provider login per daemon host, nothing forwarded; no multiple accounts or switching. Its own login: device flow, refresh session in `~/.humanlayer/riptide/`, or a short-lived launch token used for one process and never saved.
- Session state `waiting_for_usage_limit` (golden/session_state_machine.md): `running` → waiting on a rejected `rate_limit_event` with no overage; back to `running` when the window resets and the agent streams again.
- Overage (`host-usage-limits.ts`): `overage` and `seven_day_overage_included` are overage types; a rejection parks only when `overageStatus` is not allowed/allowed_warning and overage is not in use. This answers questions.md 3: a `seven_day_overage_included` rejection should not cool the account when overage is still allowed or in use.
- The Claude CLI itself waits out a rejected window, reporting each wait as `system`/`api_retry`; the SDK emits no result while parked. lf must not wait for an exit to detect the limit.
- Windows: `five_hour`, `seven_day`, `seven_day_opus`, `seven_day_sonnet`, model-scoped as `model:<display name>`; snapshot from the Agent SDK's get-usage control request; credit and codename buckets ignored. The five-hour window is still in their current schema.
- ccdaddy (third-party): daemon swaps `~/.claude/.credentials.json` account keys under Claude's own locks at a usage threshold (default 80%); running sessions pick it up on the next request.
11. **Unobserved logins.** Settled at demo (Jack, November 6): a window past its reset, one with no reset time, and a login with no weekly reading score a full week (100% over 168h); `lf account connect` also polls once.
12. **No draft signal.** Nothing records a typed-but-unsent draft, so `lf session move` treats an available Session as idle and moves it; a busy one (no availability) waits for its Stop hook. The design's "draft-holding Session listed as skipped" has no source yet.
14. **A model change starts a seeded thread.** `lf session move -a claude:opus` on a Claude Session goes through `lf session new -a`, not a native resume with another `--model`; the native thread stays in history.
15. **Items 7–9 built (November 5).** Choices made headless, each reversible:
    - Replay keeps running in the recorded capture's profile (`AccountLogin::Replayed`): switching this Machine's shared login to another catalog's account would record a switch and move every open Session.
    - `lf resume --account <email>` no longer stops a running agent; the account names the login a fresh launch switches to. Moving open Sessions was `lf session move --account` (deleted November 6; every launch now switches to the top login).
    - Rentals use the laptop-held rental login by default, with no per-rental "owned" option yet; an owned Machine is any non-rental and gets its own fresh login.
    - The laptop renews a rental's token when it is within 2 hours of expiry (tokens last ~8h); after any refresh it resends to every live rental, since whether a refresh cancels earlier tokens is unverified.
    - `provider_session_accounts.isolated` stays as "transcript still in an account home"; column removal is Task e1f359 with the long-lived token columns and `isolate:`.
16. **Resume behind an open terminal (November 5).** With `Departure::Account` gone, `lf resume --account <other> <session>` no longer ends the open terminal on the old login; `session_launch_contract`'s `a_codex_terminal_moved_to_another_login_ends_and_says_where_it_went` was deleted. A resume naming another login now attaches or waits like any resume. Whether that wait is acceptable is for review.
17. **Urgency details (November 6).** Built headless, each reversible: weekly = `weekly` or any `seven_day*` window (so `seven_day_overage_included` scores too); the tightest affecting weekly window counts; a named account list (`--account a,b`) keeps the person's order and is not re-ranked. `SWITCH_SETTLE_SECS` drops from 120 to 15: running Claude adopted a switched login within 3s in the live test, and with a switch at nearly every launch a 120s window would discard most native limit reports. A limit reported inside the window is still dropped, not attributed to the previous login.
18. **Pausing switches away.** `lf account pause <login>` on the active login switches (as `lf account use`) to the route's top other login, including Codex's daemon reconcile; with none, the login stays and the command says so.

# `lf account` CLI pass (November 6, 2026; headless, each reversible)
19. **The hold is a file**, `~/.lf/accounts/<provider>/held` (the account ID), beside the native lock, not a store column: Machine-local like the login, and no migration. It applies to default routing only; `--account`/`--only-account` still win. It ends when the held login is out on every model (cooldown, or an account-wide `limited_until`); a model-scoped limit sends only that model elsewhere and keeps the hold. A held login that is paused stops being eligible; pausing it releases the hold.
20. **`--routing disabled` has no new spelling.** `pause` is explicit-only; nothing sets `disabled` by hand now (an expired `paid-through` still behaves as paused).
21. **Local services leave the table.** Bare `lf account` shows managed logins only; `lf account linear` (or `claude`) and `--json` still show local credentials.
22. **Naming a provider is its detail.** `lf account claude` shows every Claude login in the old long form with live verification; `lf account <login>` shows one. `--details` is gone.
23. **A login argument needs `@`, `=`, a provider name or `auto`**, so a mistyped verb (`lf account status`) still exits 2 before anything runs. A bare prefix without `@` (`jack`) no longer resolves; `claude=<id>` reaches an account with no email.
24. **`use` does not count Sessions.** It says open Claude Sessions follow at their next request and Codex ones when resumed, instead of the mock's "3 open Sessions follow".
25. **`next` reasons.** Held, then the order naming only it, then "the only login ready", else the urgency reason (`31% left before Wed 12:00`). "without that order" appears only when no default order exists either.
26. **lf's own Claude refresh** takes Claude's refresh lock (settled by the concept review after the CLI pass; built in b840306cf).

# Open questions for review (c0b373)

Assumptions made in the headless implement step, each reversible.

1. **Interactive continuation.** The login moves by itself; the interrupted turn continues on the person's next message, and nothing types into a terminal. Answered at demo: shared Sessions follow the login; everything else moves through `lf session move -a`. `lf account drain` and `session move --account` are deleted (Jack, November 6).
2. **No model or harness fallback.** Overturned at demo: limits are scoped and headless steps fall through account, model, provider (Accepted at demo, items 2–3).
3. **A rejected event cools the account until its reset**, except a credits rejection (`seven_day_overage_included`, "out of usage credits"), which holds only the launched model (five whys, November 7).
4. **Waits have no upper bound.** A weekly limit can hold a step for days, in one idle process, until someone interrupts it. Stays as built (same answer).

# Jack's direction at demo (November 5, 2026)

Became the design doc's "Accepted at demo" (fall through account, then provider and model; detect every limit kind; balance across accounts). Claude and Codex only; OpenCode and Pi later in their own Task. Full text at dacc487bc.

# Account separation research (November 5, 2026)

Settled by deleting isolation (Accepted at demo, item 7); the research is in git at 400ab22ea. Still open from it: a launch with a named account list records its switch cause as `person`, so `provider_account_switches.cause` is unreliable.

# Interactive policy (Jack, November 5, 2026)

Bias against an interactive Session changing provider by itself. Bulk move: blocked Sessions by default, interactive only, never Runs; `--all` widens. "all blocked" is the most important query (history at 447a7c016). Worker fallback: an explicit agent is followed only within itself (`claude:<model>` stays; bare `claude` may fall back across Claude models and accounts); cross-provider fall-through lives in preferences. Settled shape in the design doc; full text at dacc487bc.

Live login test (November 5, Claude 2.1.318, Jack approved): fake tokens in a throwaway Keychain item, local stand-in API recording token hashes. With `.credentials.json` present, a Keychain change was not picked up (old token after 36s and 72s); touching that file's mtime moved the next request to the new token within 3s. With no such file, the next turn after 36s picked up the Keychain. Jack's `~/.claude/.credentials.json` was a 22-character test stub dated October 18; removed November 5 (Jack approved). `/login` spreading across real Sessions still to be seen in use.

# Why isolation exists (research, November 5)
Goal: several Max accounts spread work without thinking about which is in use. Isolation (pre-#1416 balancing) is deleted; weekly limits make sequential use as good as concurrent. Detail at dacc487bc.

# Assumptions while building the demo decisions (November 5, 2026)

6. **Model scope names.** Claude 2.1.318 knows `five_hour`, `seven_day`, `seven_day_overage_included`, `seven_day_opus`, `seven_day_sonnet`, `seven_day_cowork`, `seven_day_omelette`, `seven_day_oauth_apps`. Only a suffix naming a model lf can launch (`opus`, `sonnet`, `haiku`, `fable`) scopes a limit; `seven_day_omelette` cools the whole account until its model is known.
7. **A launch with no model** (bare `claude`) is held back by every scoped limit on an account, since lf cannot tell which model Claude would pick. Model fall-through then names one.
8. **Ordered preferences** are a comma list in `default_agent` (`lf set default_agent claude,codex`), mirroring account routes. The first entry is the agent; the rest are providers a headless step may fall through to.
9. **Claude model rungs** are `opus`, `sonnet`, `haiku` in that order, skipping the model already running; Codex limits are account-wide, so it has no model rungs.
10. **Busy Sessions** move at their next Stop hook rather than mid-turn.
- Remote isolation (`lf ssh`, `f72bc3ab9d` #1493) existed for a laptop-selected account not switching the target's shared login, and for SSH needing file credentials; a shared home holding a file login replaces both. Comps for remote credentials (Paseo, Omnara, Warp) forward no subscription logins; detail at 400ab22ea.

# HumanLayer comp (github.com/humanlayer/humanlayer, read November 5)
One login per daemon host, no switching. Two facts still live: `overage` and `seven_day_overage_included` rejections park a session only when overage is not allowed or in use; the Claude CLI waits out a rejected window itself, reporting `system`/`api_retry`, and the SDK emits no result while parked, so lf must not wait for an exit to detect a limit. ccdaddy swaps `.credentials.json` keys under Claude's locks at 80% and running sessions pick it up next request. Detail at dacc487bc.
11. **Unobserved logins.** Settled at demo (Jack, November 6): a window past its reset, one with no reset time, and a login with no weekly reading score a full week (100% over 168h); `lf account connect` also polls once.
12. **No draft signal.** Nothing records a typed-but-unsent draft, so `lf session move` treats an available Session as idle and moves it; a busy one (no availability) waits for its Stop hook. The design's "draft-holding Session listed as skipped" has no source yet.
14. **A Session move to another model starts a seeded thread** (`lf session new -a`); a Graph step's model fall-through keeps its native thread.
15. **Items 7–9 built (November 5).** Choices made headless, each reversible:
    - Replay keeps running in the recorded capture's profile (`AccountLogin::Replayed`).
    - `lf resume --account <email>` names the login a fresh launch switches to; a running Codex restarts on it (16); a running Claude's login switches, then it attaches (28).
    - Rentals use the laptop-held rental login by default; `lf machine connect <rental> claude <email> --owned` gives one rental its own fresh login, and renewal skips it. An owned Machine is any non-rental and gets its own fresh login.
    - The laptop renews a rental's token when it is within 2 hours of expiry (tokens last ~8h); after any refresh it resends to every live rental, since whether a refresh cancels earlier tokens is unverified.
    - `provider_session_accounts.isolated` stays as "transcript still in an account home"; column removal is Task e1f359 with the long-lived token columns and `isolate:`.
16. **Resume behind an open terminal (November 5).** With `Departure::Account` gone, `lf resume --account <other> <session>` no longer ends the open terminal on the old login; `session_launch_contract`'s `a_codex_terminal_moved_to_another_login_ends_and_says_where_it_went` was deleted. Settled at demo (answer quoted in the design doc, "Demo answers, second pass"): a running Codex restarts on the named login; Claude follows the switched login.
17. **Urgency details (November 6).** Built headless, each reversible: weekly = `weekly` or any `seven_day*` window (so `seven_day_overage_included` scores too); the tightest affecting weekly window counts; a named account list (`--account a,b`) keeps the person's order and is not re-ranked. `SWITCH_SETTLE_SECS` drops from 120 to 15: running Claude adopted a switched login within 3s in the live test, and with a switch at nearly every launch a 120s window would discard most native limit reports. A limit reported inside the window is still dropped, not attributed to the previous login.
18. **Pausing switches away.** `lf account pause <login>` on the active login switches (as `lf account use`) to the route's top other login, including Codex's daemon reconcile; with none, the login stays and the command says so.

# `lf account` CLI pass (November 6, 2026; headless, each reversible)
19. **The hold is a file**, `~/.lf/accounts/<provider>/held` (the account ID), beside the native lock, not a store column: Machine-local like the login, and no migration. It applies to default routing only; `--account`/`--only-account` still win. It ends when the held login is out on every model (cooldown, or an account-wide `limited_until`); a model-scoped limit sends only that model elsewhere and keeps the hold. Pausing the held login releases the hold; `lf account use` on a paused login holds it anyway (30).
22. **Naming a provider is its detail.** `lf account claude` shows every Claude login in the old long form with live verification; `lf account <login>` shows one. `--details` is gone.
23. **A login argument needs `@`, `=`, a provider name or `auto`**, so a mistyped verb (`lf account status`) still exits 2 before anything runs. A bare prefix without `@` (`jack`) no longer resolves; `claude=<id>` reaches an account with no email.
25. **`next` reasons.** Held, then the order naming only it, then "the only login ready", else the urgency reason (`31% left before Wed 12:00`). "without that order" appears only when no default order exists either.
27. **Fixture isolation is one ctor, not a `CODEX_HOME` per fixture (November 7).** About 45 test files spawn `lf`, and fixtures that set `HOME` expect `<HOME>/.codex`, so a process-wide `CODEX_HOME` would break them. Moving `HOME` covers `~/.codex` and `~/.claude` for every inheriting fixture. Fixtures without their own `HOME` share one home per test binary.
28. **`lf --account <other> resume` on a running Claude switches the login, then attaches** (concept review, November 7; was: attaches and drops the name). It moves every open Claude Session, as any launch does.
29. **A running Codex with no recorded account** (no `provider_session_accounts` row) counts as a different login, so naming any login restarts it.
30. **A hold beats repository and Wave orders (concept review, November 7).** Answer 3a has no exception and the page shows `use` right after an order naming another login. Reversible: the other reading is order wins and `use` says so.

## Item 12 (November 7, 2026, headless)
- Assumption: the matcher takes only the three openers named for item 12. `xAo`'s other spent lines (`You're out of extra usage`, `Your org is out of usage · …`, `Your group's usage limit is set to $0`) move nothing, which fails toward no switch. Adding them is one line each in `subscription_limit_text`.
- `You've reached your Fable limit.` matches through its opener, though Claude types it `model_requires_usage_credits`; item 13 scopes it to Fable.

- **Codex stated limits are not polled (item 14, November 7).** `record_stated_limit` polls only Claude accounts after a text-only limit. Codex's poll is a `codex app-server` exchange of up to 60s and its stream already reports `rate_limits` windows, so a Codex Graph step whose limit only stderr reported keeps the 15-minute cooldown. Reverse by dropping the provider check if Codex steps are seen cycling back onto a spent account.

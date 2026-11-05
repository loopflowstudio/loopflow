# Open questions for review (c0b373)

Assumptions made in the headless implement step, each reversible.

1. **Interactive continuation.** The login moves by itself; the interrupted turn continues on the person's next message. Nothing types into a terminal, to keep active input safe. Automatic continuation of idle Sessions would need a reliable "no draft in progress" signal.
2. **No model or harness fallback.** Preferences hold one resolved agent with no ordered alternates, so any fallback would invent policy. A model-scoped limit cools the whole account.
3. **A rejected event cools the account until its reset**, including `seven_day_overage_included`. If overage rejection leaves the plan's own quota usable, this cools too much.
4. **Waits have no upper bound.** A weekly limit can hold a step for days, in one idle process, until someone interrupts it.
5. **Isolated Session resume** no longer passes its recorded account as exact, so a cooled account moves the conversation instead of refusing. The pin still wins whenever that account is eligible.

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

Design comps (November 5): Rex 0.1.0 has no query or filter on its lists. It offers a shared target resolver (`target.resolve`: ID, label, position), `ls -q` IDs for shell composition, variadic `kill SESSION...`, and a named action registry (`rex actions`, `rex do`) that CLI, key bindings and Lua all invoke. Comps from general knowledge, not rechecked: Taskwarrior (`task +BLOCKED modify ...`: filter first, computed virtual tags, confirmation), kubectl (`-l` shared by get and delete; `cordon`/`drain`), docker (`ps -f`, `-q`), Gmail/Linear (filter, select all, act). Open idea: drain the account rather than move Sessions.

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

---
layout: default
title: Subscription Management
---

# Subscription Management

Connect Claude and Codex logins once, then route each repository through them:

```bash
lf account connect claude=personal@example.com --chrome-profile personal@example.com
lf account connect codex=work@example.com --chrome-profile work@example.com
lf account

lf set account.claude personal@
lf set account.codex work@,personal@
lf account next
```

```
$ lf account
  claude  personal@  ━━━━━───────────   31% left  Wed 12:00  ● active
                     fable out until Wed 12:00
  codex   work@      ━━━━━━━━━───────   60% left  Wed 16:04  ● active
  codex   spare@     ╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌    ? left             14h ago

  ! codex spare@ needs a login   lf account connect codex=spare@
```

Loopflow manages Claude and Codex subscription logins as separate identities.
OpenCode Zen, GitHub, and Linear each use one effective credential instead of a
routable subscription catalog.

`lf account` shows one row per stored login: its weekly window as a bar and
percent left, when the window resets, `● active` for the login the provider's
home is signed in as, `held` and `paused`, and a reading's age once it is more
than 15 minutes old. It reads the cache and asks the provider only about logins
whose reading is stale. `?` means unknown, never zero. Lines starting with `!`
name something to fix and the command that fixes it. `lf account --cached`
contacts no provider and writes nothing.

A login is its email or an unambiguous prefix (`work@`). When Claude and Codex
both have one by that name, say which: `claude=work@`, the same spelling
`--account` takes.

## Connect an identity

`lf account connect` registers an existing provider login. It does not create a
Claude or Codex account.

```bash
lf account connect claude=personal@example.com --chrome-profile personal@example.com
lf account connect codex=work@example.com --chrome-profile work@example.com

lf account connect claude=personal@example.com --import # adopt the ambient Claude login
lf account disconnect personal@
```

Connect creates a **local managed identity**, not a new Claude or Codex account:

1. Loopflow derives a stable, path-safe account ID from the requested login.
   The ID is a storage key, not another username or credential.
2. The provider login runs in a private staging home. The chosen Chrome profile
   opens the provider's authorization page, but its browser profile is not
   copied.
3. Loopflow matches Codex's `account/read` email to the staged credential and
   reads its per-user subject from that credential. For Claude, it reads
   `/api/oauth/profile` account email and UUID. The observed email must match
   the requested email. A login already held by another managed account is
   refused, naming both accounts.
4. The verified provider credential moves into
   `~/.lf/accounts/<provider>/<account-id>/`.
5. Loopflow writes the observed email, user identity, plan and operating state to
   `~/.lf/loopflow.db`, then reads the login's usage once, so routing ranks it
   on its real quota from its first launch.

Codex requests a callback URL from `account/login/start` in its app-server,
which disables Codex's own browser opening. Loopflow opens that URL in the saved
Chrome profile and waits for the matching login completion before checking and
installing the staged identity. Cancellation or failed login keeps the existing
credential.

Claude opens its native browser callback route and completes after approval.
If the callback cannot reach this terminal, type `m` then Enter to open the
provider's manual route in the same profile; code input is hidden. Headless
connect never consumes stdin as an authorization code.

Select a Chrome profile once for each target:

```bash
lf account connect linear --chrome-profile Work
lf account connect linear              # reuse Work
lf account connect personal@           # reuse this account's profile
```

Saved names and ordered profile choices remain reusable. A first interactive
connection offers local Chrome profiles; headless connection requires
`--chrome-profile` when no choice is saved. A new choice need not have a Google
login. Existing expected-login constraints remain enforced. Browser selection is
remembered only after successful authorization, independently for each target.

If verification fails, the staging login is discarded and an existing identity
is left unchanged. `lf account connect claude=<email> --import` is the explicit
exception: it copies
the ambient Claude login from `~/.claude` or the macOS Keychain into a new
stored account profile, then performs the same login verification.

The provider home holds the provider CLI's authentication and session state.
Loopflow may link shared settings, skills, and plugins from `~/.claude` or
`~/.codex`; it does not share credential or session files between managed
identities. The database stores health, routing state, usage observations,
access-profile bindings, and provider-session pins.

Commands identify an account by its full login email or an unambiguous email
prefix. The path-safe internal account ID is a storage key, not a user-facing
selector. An older account with no email can be selected by its ID solely to
set its expected email before reconnecting. `--login-email` refuses to relabel a
credential whose observed email disagrees.

An access profile records which Chrome profile can authenticate an identity:

```bash
lf account set personal@ --chrome-profile Personal --chrome-profile Work
lf account set linear --chrome-profile Work
lf account personal@
```

The profile is an authentication venue, not the identity that spends provider
usage. It is never a run-time account selector.

Claude and Codex conversations run in the provider's ordinary home, signed in as
one stored account at a time, so plain `claude --resume` and `codex resume` find
them. See [Switch the shared account](#switch-the-shared-account). Stored
profiles under `~/.lf/accounts/` hold logins to switch between; nothing runs
in them. Loopflow records health and the account each conversation ran on. A
conversation that ran in an account's own home, before every launch shared one,
moves into the shared home when it resumes.

## Inspect account state

```bash
lf account                       # one row per login; stale rows refresh
lf account work@                 # that login: auth, plan, each window's age and source, its browsers
lf account claude                # every Claude login and Claude's local credentials, with every saved browser
lf account claude --json         # refresh Claude usage and inspect saved limits
lf account --cached              # offline: no provider contact, no writes
lf account --json                # every login and local service as one document
```

Naming a login or a provider asks the provider about each one shown.

`lf account` shows one row per login, then a line per thing to fix with the
command that fixes it. Naming a login shows that login alone: its auth, plan,
each usage window with its age and source, and the browser profiles it
connects through. Naming a provider shows stable account IDs beside full
logins for every login. Codex rows show the credential's observed login, and report mismatches
or shared logins with a reconnect command even during cached inspection.
Verification only accepts usage after the server and native identity agree.
The provider form lists managed accounts and local service credentials in
separate sections: an expired local token says nothing about a managed account.
Missing local tokens leave ambient auth uninspected. Cached inspection reads
account state without starting a provider, decrypting local tokens or creating
an encryption key. The CLI records the command's Process in its Machine; this
can initialize an empty store, but creates no account, route or conversation.
Local token metadata is cached evidence, not server acceptance; live status
reports local server verification unavailable.

`lf account` persists recognized managed subscription windows before
printing percentages, reset times and plan. Each window keeps its own observation
age and source; omitted or unavailable windows retain older evidence. A passed
reset displays usage as unknown until refreshed. Current windows say both
`N% used` and `M% left`. JSON retains dated window observations; consumers must
check `resets_at` before treating a recorded percentage as current. `lf session usage`
reports recorded provider token/cost usage separately.

JSON contains `accounts` and optional `browser` details. Each row has `scope`
(`managed` or `local`), provider, account ID, login, cached
credential state, verification, windows, diagnostic, recovery,
`expires_at` (a rental's access token, which the laptop renews), `active` and
`held`. Verification
is `accepted`, `rejected`, `unavailable`, or `not_checked`. Window rows retain
`observed_at`, `resets_at`, source and plan; `verified_windows` identifies only
this invocation's new observations. Null means unknown. A successful report
does not imply successful authentication. Cron-host requires accepted managed
evidence:

```bash
lf account --json |
  jq -e 'any(.accounts[]; .scope == "managed" and .verification == "accepted")'
```

Claude's cached login metadata does not establish a freshly observed plan.
Until the usage response supplies confirmed plan evidence, its new observations
leave plan unknown. Codex retains the plan from `account/read`, including when
no usage windows are returned. Healthy automatic Codex candidates prefer an
observed Pro plan, then Plus; explicit preferences and Session pins take priority.
A plan does not establish remaining capacity.

Cached account inspection starts no provider and changes no stored state. Missing
credentials remain visible with older usage and the `lf account connect` recovery
command. Unreadable credentials are distinguished from missing files. Verification
records provider rejection as missing; locally detected identity mismatches are
reported without changing cached state. Unavailable usage leaves credential state
unchanged. Neither changes account configuration. Provider routes skip missing,
disabled and cooling accounts, and accounts limited for the requested model,
then continue to the next candidate.

After a Claude limit, run `lf account claude` to refresh usage. If Fable now
has room, its fresh weekly reading replaces the older Fable rejection. If
Fable is still at 100% but the account-wide windows have room, Fable waits
while Sonnet, Opus and Haiku remain eligible, including when reopening a
conversation on those models. Account details show which model is held and
until when. In JSON, `cooldown_until: null` means no account-wide cooldown;
individual windows can still hold a model back.

A refresh clears an old rejection only when the corresponding usage window
establishes recovery. Overall weekly room alone does not clear a Fable limit;
Fable room does not clear an account-wide limit. Included quota does not clear
paid-credit exhaustion. Failed, empty or incomplete readings retain limits
they cannot resolve, as do rejections whose cause is unknown. Successful
authentication alone establishes no capacity. A rejection or other capacity
update received while a refresh is in flight takes precedence over that
refresh; the report shows the saved result, and another refresh can check
again. Discarded readings are absent from `verified_windows`.

A credential replacement detected during verification makes that result
unavailable and retains prior account evidence. A switch holds Claude's own
refresh lock (`<home>/.oauth_refresh.lock`) while it saves the outgoing login
and installs the next, so a running Claude cannot refresh in between; status
reads do not take it.

Eligible accounts are ordered by plan, then by how fast each must be used to
spend its quota before it resets: Codex Pro leads Plus, then the account with
the most weekly quota left per hour until its weekly window resets leads, so no
account resets with quota unused. 70% left with 3 hours to go leads 40% left
with 5 days to go. The weekly window counts its model's own limit when there
is one; the short window decides only whether an account has room. A window
past its reset, and an account with no weekly reading, count as a full week to
spend (100% over 168 hours); declared route order decides ties. Every launch takes the top account, whichever is signed in,
so the shared login switches, and every running Claude follows, whenever
another account ranks ahead. A resumed conversation runs on the active login;
the account it began under is history. A named login that cannot launch fails
rather than falling back to another account.

Claude's cached identity is bound to the credential bytes checked by the profile
endpoint. Cached status reports changed or previously unverified credentials as
unknown; stale `.claude.json` metadata never establishes identity. Live status
and connect save that binding. Routing checks expected email, user UUID and
duplicate logins, requesting a profile observation when the credential changed.
Recorded Claude and Codex launches both require their owning account catalog.

Account state remains in the Machine database.

Take a login out of rotation, or bill it:

```bash
lf account pause personal@                   # out of rotation; --account still launches it
lf account unpause personal@
lf account set personal@ --paid-through 2026-08-14
lf account set personal@ --clear-cooldown
```

Pausing the active login switches the Machine to the route's next login at
once, and every open Claude follows; with no other login, it stays and says
so. Clearing cooldown retains credential state, aggregate utilization and every
observed window. The command reports any window holds that remain, including
their models and deadlines. It does not reset provider quota. Once `paid-through` passes,
an otherwise automatic login behaves as paused until the date is cleared.

## Route a repository

An account route is an ordered list of subscription logins to try for one
provider in one repository, one Wave or every repository:

```bash
lf set account.claude personal@,work@
lf --wave billing set account.codex work@
lf set account.codex work@ --global
lf unset account.codex
lf account next
```

```
$ lf account next
  claude  personal@example.com     the only login ready for this launch
  codex   manabot-eng@example.com  this repository's order names only it
                                   without that order: work@example.com, 60% left
```

`next` explains account routing for each provider without choosing a model.
It conservatively treats a limit on any model as a blocker, so it can report
Claude blocked by Fable while a Sonnet launch on that login is eligible.
`--agent` does not make `next` a model-specific check. Fresh launches and resumed
conversations check capacity for the model they run.
It exits 0 when both providers are ready, 2 when one is blocked until a reset, and
3 when one needs a login. `--json` reports the same per provider, with
`outcome` `ready`, `blocked`, `login_needed` or `ambient`. `--default` ignores
repository and Wave orders. An order that names fewer logins than are in
rotation shows in `lf account` as a `!` line.

Account order uses the personal global, repository or Wave scope. Repository and
Wave declarations live in the clone's `lf.config.local.yaml`; linked Task worktrees
share that file, while independent clones can choose different orders. Global
orders live in `~/.lf/config.yaml`. `set`, `unset` and hand edits reach the same
configuration; no DB override sits above the file.

The list contains account references, not credentials or network routes. Account
registration, observed eligibility, usage and conversation attribution retain their
runtime owners. No per-skill account policy exists. For remote work, the initiating
profile supplies account preferences and the executor supplies its own credentials.
See [Configuration](config.md#remote-work) for forwarding and retained preferences.

A managed launch tries the repository route, then the default provider route.
Without either route, every automatically eligible managed identity is a
candidate. With no managed candidate, the provider CLI can use its ambient
default login.

## Select accounts for one launch

Each launch records `provider_account_selected` with the actual account and
attempt, including headless Task steps before a provider session ID is available.
Later attempts retain earlier account evidence; a null account means ambient
execution. Once the provider reports its native identity, continuation must retain
that identity and selected account/native Machine. Requested account and actual
selection are different evidence; [cutover status](dev/reference-cutover.md#cutover-status)
records the remaining history/account conversion.

Prefer an account while keeping the normal route as fallback:

```bash
lf --account codex=work@ implement
lf --account claude=personal@ --account codex=work@ review
```

Restrict the process tree to exact accounts:

```bash
lf --only-account claude=personal@ review
lf --only-account claude=personal@ --only-account codex=work@ implement
```

`--account` and `--only-account` are repeatable. An unqualified selector is
resolved independently for Claude and Codex. The two flags cannot be combined.
A provider omitted from `--only-account` is unavailable to that process tree.

Choose both providers once for a Graph or Task:

```bash
lf --only-account claude=personal@ --only-account codex=work@ graph code
lf --account claude=personal@ --account codex=work@ task run LOO-123
```

A Graph carries these choices to its steps. `--account` keeps fallback routing;
`--only-account` restricts the entire Graph. Run a fresh Graph to choose again.
Steps have no account overrides. Native Session affinity remains authoritative within that selection.
Local choices use the Machine's catalog directly and survive the initiating CLI's
exit, including logins installed on a remote machine.

## When an account runs out

A Graph step keeps going by itself:

```bash
lf task status LOO-1
#   implement went on under another claude account after: provider_rate_limited: rejected: five_hour
#   compress went on under claude after: codex_error: You've hit your usage limit
#   implement is waiting for claude capacity until Nov 5 14:00: ... It goes on by itself.
```

| What happens | What the step does |
|---|---|
| Its account hits a limit, another is eligible | Continues the same conversation under that account |
| Every eligible account is held back and `agents` has a next entry (`agents: [claude:fable, claude:opus, codex]`) | Continues on that entry: the same conversation on another model of its provider; on another provider, seeded with the conversation so far, as `lf session new -a` does |
| Every eligible account is held back, no `agents` order is behind it and its harness was named bare (`claude`) | Continues the same conversation on another model of that harness: Opus, then Sonnet, then Haiku |
| The provider refuses the launch itself with a usage limit, before any turn | The same moves: next login, then the next entry in `agents`. If another entry takes the step, the run continues; if the order runs out, it stops once |
| The provider reports that the model is unavailable | Moves directly to the next allowed entry, without spending the login’s quota. An explicit model pin stops once with advice to change the pin |
| Nothing it may use is free, and an account has a known reset | Waits, checking each minute, and continues at the first reset |
| A usage limit or unavailable model refuses it and no agent it may use is left | Stops once, without automatic retries. The stop line and `lf task status` say what would let it move, such as `lf set --global agents codex,claude`. After a reset or an `agents` change, `lf task run <ID>` starts at that step; earlier steps stay done |
| Its Process ends during a wait | `lf task run` starts the run again at that step, and earlier steps stay done. An assigned Task that nothing else blocks restarts by itself after the reset, on the `-a` the run was given; no automatic restart starts it sooner |

Only accounts in the resolved route take a step on. An `agent` choice or `-a`
keeps the provider, and the model if given. Otherwise,
[`agents`](config.md#keep-going-when-a-provider-runs-out) supplies the order:
`lf set --global agents claude,codex` lets a step move from Claude to Codex.
A launch refused on a login Loopflow does not route fails with its limit once
the last entry refuses too. Nothing is written to your preferences.
Commit messages and PR text fail at once, because someone is waiting for them.

A [saved Claude model](config.md#claude-model) supplies bare `claude`'s starting
model without pinning it. For example, `claude.model=opus` makes routing check
Opus capacity while keeping the model and provider fallthrough described above.

A limit the provider scopes to one model, such as Claude's weekly Opus or Fable
limit, holds back only launches of that model: Sonnet on the same account keeps
going, and a launch with neither an explicit nor a saved model waits for every
scope on the account.
A limit Claude only states in words ("You've reached your Fable limit") is
scoped the same way. Loopflow then reads that account's usage once, and when
the read fails the model is held for 15 minutes, plus a five-second grace
period. A spent window with a known reset holds that window's launches back
until five seconds after the reset. A missing reset time stays unknown.

Claude's `seven_day_overage_included` is an included weekly allocation for a
model, separate from paid usage credits. Loopflow uses the reported model, or
the launch's model when Claude omits it, to scope that limit. A fresh weekly
reading for that model replaces the older rejection even if Claude uses a
different window name. Unrelated or unknown limits remain in place.

A stated credit failure ("You're out of usage credits") holds back the model
the launch ran. Included-quota readings do not clear that credit failure.
Bare `claude` uses the saved Claude model, or Fable by default. With no saved
model or other configured entry behind it, a Fable limit lets the step go on
with Opus on the same account, in the same conversation, if Opus has capacity.

Open Claude Sessions need nothing either. When one hits a limit, the login
moves to the eligible account that ranks first, and every open Claude follows
at its next request. Send your next message where you left off. A running
Codex keeps its login until you `lf resume` it. Naming another login with
`lf --account codex=<login> resume <session>` restarts it there, and the old
terminal says where the conversation went. A running Claude stays put:
`lf --account claude=<login> resume <session>` switches the Machine's login,
which every open Claude follows, and attaches.

To see what would fail next, and move it in one go:

```bash
lf session list --blocked                       # Sessions whose login cannot take their model now
lf session list --on claude:fable               # every open Fable Session
lf session list --on codex=work@                # every open Session using that login
lf session move --blocked -a claude:opus        # continue them on Opus: idle ones at their next open, busy ones after their turn
lf account pause jack@                          # stop routing to a login; when it is active, the Machine switches off it now
lf account unpause jack@                        # route to it again
```

`--blocked` and `--on` are the one Session selector; `list` previews what
`move` acts on. A move never writes preferences. Taking the active login out
of automatic routing switches the Machine to the route's next login, and every
open Claude follows; `lf account use` switches by hand and holds there. In the Desktop, View → Move Blocked Sessions… runs the same move.

## Switch the shared account

```bash
lf -a codex : "say hi"                    # runs in ~/.codex as the active account
codex resume                              # plain Codex sees that conversation
lf account use codex=work@                # ~/.codex is now work@, for lf and codex
lf account                                # ● active marks it
lf account use claude=work@               # the same for ~/.claude and plain claude
lf account use auto                       # launches go back to the top-ranked login
```

```
$ lf account use work@
work@ is both claude and codex: lf account use claude=work@
$ lf account use codex=work@
codex is now signed in as work@example.com; open Sessions follow when resumed
codex stays on work@example.com until it fills up, or lf account use auto
```

`lf account use` signs the provider's home in as a stored login and holds every
launch of that provider there, whatever a repository or Wave order says and
paused or not, until the login is full (a cooldown or a limit on every model) or
`lf account use auto` releases it. A limit on one model sends only that model's
launches to the order, and `--account` still wins. Pausing the held login also
releases it.
It exits 2 when the login is out until a reset or names no stored login, and 3
when it needs a login, changing nothing. It first saves the current login back to its stored account,
so a token the provider refreshed while active is kept; a login Loopflow has
never seen is kept as a new paused account.

When Codex's native and stored copies name the same login but disagree, live
status checks the native copy first. Only a definite authentication rejection
allows checking the stored copy. Launches repair rejected credentials in the
original home after verifying the other copy; conversations and resume IDs stay
there. Displaced files remain in private `.lf-auth-recovery-*` directories beside
`auth.json`, including late writes from an already-running client. Timeouts and
service failures report verification unavailable and leave both copies intact.
Cached status performs no provider requests. Recovery never restarts a daemon.

For Codex, `use` also restarts its shared background app-server if it still holds
another login, even when the file was already correct. Running turns get up to
five minutes to finish; a turn still running at restart is interrupted. The login
is installed before that wait, so new lf launches need not wait. lf's Codex
terminals run their own private app-server and keep their login until restarted.
An unreadable daemon login returns an error without restarting it; the selected
login remains installed. Failed activity checks keep the grace period rather than
ending it early. Expiry warns that restarting may interrupt remaining turns.
Cancelling during the wait leaves the installed login in place; repeat `use` to
reconcile the daemon later.

| | Codex | Claude |
|---|---|---|
| Where the home's login lives | `auth.json` | macOS Keychain item; `.credentials.json` elsewhere |
| An lf process already running | keeps the login it started with until it restarts | follows the switch |
| A turn in flight during a switch | may fail once; a headless run resumes it under the new account | continues |
| Whose login the home holds | read from the login | asked of Claude once after Claude refreshes it |

A Claude login names no person. After Claude refreshes the home's login,
Loopflow asks Claude whose it is before the next launch or switch and brings
that account's stored copy up to date. If it cannot ask, a switch keeps the
unrecognized login beside the stored accounts rather than discarding it.

A shared launch moves the home to the eligible account that ranks first, even
when the active account could take it.
A parked Claude login whose access token has expired must refresh first: a
rejected grant leaves it needing a login, and an unreachable token endpoint
skips it for that launch. Open Claudes stay where they are either way.
`--account codex=work@` makes that account active before launching.
These launch-time switches do not restart Codex's shared daemon. If plain
`codex` keeps the old login, run `lf account use codex=<login>` with the selected account.

Every launch runs in the shared home; `isolate:` in `.lf/config.yaml` is read
and ignored. Load spreads across accounts over time, as each launch takes the
login with the most quota to spend before its reset.

Codex's own conversation ID works wherever a Session ID does:

```bash
lf resume 0199a213-81c0-7800-8aa1-bbab2a035a53           # the ID `codex resume` takes
lf session history 0199a213-81c0-7800-8aa1-bbab2a035a53
```

Resuming a conversation plain Codex started brings it in as a Session with
no Task. Normal batch completion and closing the current connected interface
close its Codex process, leaving saved history available to plain `codex resume`.
`lf resume` reconnects to a running conversation without restarting it;
the old attachment's exit cannot close the new owner's Codex process. Codex allows one
writer per conversation, so use `lf resume` while it is still running.

`lf account --json` reports `active` per login.

A Codex home whose `config.toml` sets `cli_auth_credentials_store` to anything
but `file` cannot be switched; set it to `file`.

## Resets

Claude and Codex both bank resets: one-time grants that clear a usage window
before its date. See what a login holds, then spend one:

```bash
lf account claude=personal@
lf account redeem-reset claude=personal@ --scope fable
```

```
$ lf account claude=personal@
  …
  weekly: 70% used, 30% left · live · resets 2026-11-18T20:00:00Z
  weekly:fable: 100% used, 0% left · live · resets 2026-11-18T20:00:00Z
  resets: 2 usable (live)
    weekly  full-reset-2611        1 of 1 left · usable now · clears weekly (70% used), weekly:fable (100% used) · ends 2026-12-10
    fable   launch-2610-fable-6mo  1 of 1 left · usable now · clears weekly:fable (100% used) · returns it to the weekly limit · ends 2027-03-09
    fable   launch-2610-fable      0 of 1 left · unavailable · clears weekly:fable (100% used) · returns it to the weekly limit · ends 2027-03-09

$ lf account redeem-reset claude=personal@ --scope fable
fable · launch-2610-fable-6mo · weekly 70% · weekly:fable 100%
Reset attempt 4b0e9c1e-…; retry with --idempotency-key 4b0e9c1e-… --reset-id launch-2610-fable-6mo
reset · fable · launch-2610-fable-6mo
  before  weekly 70% · weekly:fable 100%
  after   weekly 61% · weekly:fable 0%
```

The next launch that asks for Fable can take this login again. Nothing else
needs clearing.

### What a login holds

Live `lf account <login>` lists each reset with its scope, ID, how many are
left, whether it can be spent now, the windows it clears with their current
usage, and when it ends. A reset that can't be spent says why, in the
provider's word (`cooldown`, `needs_limit`, `unavailable`, …).

| Scope | Provider | Clears |
|---|---|---|
| `weekly` | Claude | The overall weekly window, and the model windows inside it |
| `fable` | Claude | Fable's weekly window, returning what Fable used to the overall weekly limit |
| `session` | Claude | The five-hour window |
| `full` | Claude | Session and weekly together |
| the credit's `reset_type` | Codex | What Codex says that type clears |

| Line | Meaning |
|---|---|
| `resets: 2 usable (live)` | The provider's count, read just now |
| `resets: 0 usable (live)` | Resets exist but none can be spent; each line gives its reason |
| `resets: unavailable (tier)` | Claude says this login can't use resets, and why |
| `resets: not reported` | The provider answered without reset information. Unknown, not zero |
| `resets: unknown` | `--cached`, which asks no provider |

Codex may return a count without the list behind it; the count stands. Logins
Loopflow doesn't manage, and Claude API keys, have no resets line.

`lf account --json` carries the same record for both providers in each row's
`resets`: `usable`, `unavailable`, and `resets[]` with `id`, `scope`, `label`,
`left`, `total`, `usable_now`, `unavailable_reason`, `clears`,
and `expires_at`. `clears` is null when the provider does not report it
(Codex); otherwise it uses the window names of the
row's `windows` (`seven_day`, `seven_day_fable`). A null `resets` is unknown.

### Spend one

```bash
lf account redeem-reset claude=personal@ --scope weekly
lf account redeem-reset claude=personal@ --reset-id full-reset-2611
lf account redeem-reset codex=work@example.com
```

`redeem-reset` takes one managed login and spends one reset. `--scope` takes
the first usable reset of that scope in the provider's order; `--reset-id`
names one. With neither, a Claude login whose usable resets share one scope
spends from it, and one with several scopes stops, lists them and spends
nothing. Codex chooses its own credit when given neither; `--scope` filters by
`reset_type` when Codex returned the list.

The provider lets a reset be spent before any limit is reached, and so does
`redeem-reset`: naming the login and scope is the decision. It prints the reset
it chose and the usage that reset would clear, then the attempt line, then
sends the request. Most resets come one to a grant, so read `lf account
<login>` first. Status, routing and Graph launches never spend one.

After the provider answers, Loopflow reads the login's usage again and records
it, exactly as `lf account` does. That reading is what frees the login:
launches route to it again, and a resumed conversation uses it. A
window omitted from that reading keeps its saved observation. A limit reported
while the reset is in flight takes precedence; the result says a newer
observation was kept.

| Outcome | Spent | Meaning |
|---|---|---|
| `reset` | yes | The windows are cleared |
| `alreadyRedeemed` (Codex), `already_used` (Claude) | no | This attempt, or this reset, was spent earlier |
| `nothingToReset` (Codex), `not_limited` (Claude) | no | The provider found nothing to clear |
| `noCredit` (Codex) | no | No credit is left |
| `cooldown`, `ineligible`, `unavailable` (Claude) | no | Refused; the line adds the provider's reason, such as `expired`, `paused` or `not_next_grant` |

These are the provider's words, unchanged, and each exits 0. `--json` prints
`login`, `scope`, `reset_id`, `outcome`, `reason`, `idempotency_key`, `before`,
`after`, `refresh_error` and `newer_observation_kept`.

### When it doesn't go through

| What happened | What you see | Next |
|---|---|---|
| No usable reset, or none in that scope | An error listing the resets and the scopes that are usable. Nothing sent | Pick a listed scope, or wait out the reason shown |
| Resets not reported or unavailable | An error with the provider's reason. Nothing sent | — |
| The reply was lost: a timeout, a server error, a rate limit or an answer Loopflow doesn't recognize | An error naming the key and reset ID. The spend is unconfirmed | Run the printed retry command |
| The login was refused | The login needs signing in | `lf account connect <provider>=<login>`, then the retry command |
| The usage read after a confirmed outcome failed | The outcome, with after-usage unknown and the read's error | `lf account <login>` |

Each attempt prints its key before the request goes out:

```bash
lf account redeem-reset claude=personal@ --idempotency-key <key> --reset-id launch-2610-fable-6mo
```

A retry with the same key and reset ID repeats the same request instead of
making a new one, keeping the provider's attempt identity. A Codex retry
needs only the key. Loopflow never guesses capacity: a spend it couldn't
confirm changes nothing until a usage read shows it.

Codex's outcomes and retry rules come from the
[Codex app-server protocol](https://learn.chatgpt.com/docs/app-server#8-earned-rate-limit-resets-chatgpt).
Claude publishes none; Loopflow uses the same operation as Claude Code's
at-limit dialog.

## Use subscriptions over SSH

```bash
lf machine connect mini github
lf machine connect mini codex work@example.com
lf machine connect mini claude personal@example.com --chrome-profile Personal
lf machine connect mini opencode
lf machine connect mini linear
lf --machine mini --account codex=work@ implement
```

Add the machine first with `lf machine add`. GitHub installs the laptop's selected
token in the target's gh configuration. Codex and Linear mint a fresh login
on the laptop; approve it in the laptop's browser. Codex stages the login
in a private temporary directory. The laptop's existing login is not replaced.
The target retains its own refresh credential and existing accounts.
OpenCode forwards its active login (OpenCode 2 on both sides): a Console login
gets a fresh device grant approved in the laptop's browser, an API key is
copied. OpenCode Zen must accept it; a different working OpenCode login on the
target is left unchanged.

Claude on an owned Machine works the same way: a fresh full login, approved in
the laptop's browser, which the Machine holds and refreshes as its own, so
`/usage` and every other feature work there.

A rental gets an access token instead, so renting needs no browser approval:

```bash
lf machine connect rental-3 claude personal@example.com   # first time: approve the laptop's rental login
lf --machine rental-3 account                             # access token until <time>
```

The laptop holds one Claude login per account for rentals, apart from its own,
and never sends it. Each connect refreshes it when its access token is within
two hours of expiry and sends the access token only; a rental keeps it in its
shared home, where running Claude picks it up. A Claude access token lasts
about 8 hours, so a rental's Claude stops about that long after the laptop last
renewed it. A connect, by hand or before a launch, sends the next token once
the rental's is within two hours of expiry, and so does the install schedule
every 2 hours (`lf machine credentials renew`; nothing when no rental is
live). After a refresh, the laptop sends the new token to every live rental at
once. The rental registers the identity the laptop verified. Only `sk-ant-oat`
subscription tokens are accepted; an API key bills per token. A background run
skips an account whose rental login still needs an approval, and `-b --rent`
refuses before renting when no account has one.

```bash
lf machine connect rental-3 claude personal@example.com --owned   # its own login; runs without the laptop
```

`--owned` gives one rental what an owned Machine has: a fresh full login, one
browser approval, that it refreshes itself. It replaces the access token, and
renewal leaves that rental's login alone.

An account-selected launch connects the requested account if missing. Invocation selectors resolve
against the laptop's managed logins;
full email identifies that login on the target. The remote invocation is restricted
to the requested accounts, so it cannot fall back to a different login. A simple
launch without selectors uses the laptop's configured provider route when available.
Read commands without account selectors do not connect accounts.

Transfers run in the foreground. `--batch` never starts a missing login, even
with a terminal attached. A headless launch needing a missing login reports
the connect command; it does not start browser authorization. SSH launches run
in the target's shared provider home, switched to the selected login like any
launch; without a Keychain login there, Claude keeps it in `.credentials.json`.
The target needs its own
provider CLI. Credential bytes travel on SSH stdin, never in argv or an exported
variable. `machine credentials inspect/receive` supplies the public receiving
commands. No broker, forwarded socket or laptop process owns the installed login.

`lf --machine` runs the target's `lf` in the saved repository. The target parses
the command; there is no extra `-- lf`. Remote process survival and reattachment belong to the separate detached-launch and relay work.

## OpenCode Zen

OpenCode Zen uses one credential rather than the managed subscription route:

```bash
lf account connect opencode
lf account connect opencode --api-key
```

Its stored credential applies to local OpenCode launches.
Subscription polling, repository account routes, and login ranking do not apply.

## Rust API migration

This auth consolidation changes the published Rust library's source API. Crate
consumers must migrate; the removed interfaces have no compatibility aliases.

- Call `ProviderAuthService::start_auth(provider)` and `disconnect(provider)`
  without an event sink. `AuthEvent`, `AuthEventSink` and `no_event_sink` are
  removed. After opening the returned browser URL, await
  `wait_for_auth(provider)` once to receive that attempt's completion, including
  persistence errors. Do not use cached `status()` as completion evidence.
- Construct `AuthFlowResponse` with `completion: AuthCompletion`. Use
  `supports_authorization_code()` and `pending_supports_authorization_code()`
  in place of the former `requires` methods. Manual input support does not mean
  authorization requires a code. Chrome authorization-code scraping is removed;
  use provider completion or explicit manual submission.
- Replace `AccountAccessProfile` with `AuthBrowserBinding`. Its `account_id`
  is optional: `Some(id)` selects a managed account; `None` names local service
  scope. `AccessProfile::expected_login` is also optional. Store and SqliteStore
  expose `set_auth_browser_profiles` / `list_auth_browser_profiles` in place of
  the account-access methods. In the list method, a missing account filter lists
  both scopes; filter binding rows when selecting a local service.
- Include `plan: Option<String>` and `resets: Option<Resets>` when constructing
  `SubscriptionUsage`. Read
  the account's stored `home` instead of the removed `subscription::account_home`
  path constructor.
- Include `observed_credential_digest: Option<String>` in `ProviderAccount`.
  Pass the optional credential digest to `record_provider_account_identity`;
  an absent digest retains historical identity without asserting a Claude
  credential binding.
- Account selection lives in `provider_account::selection`; the SSH credential
  broker and lent-account API are removed. Launch local commands with `--account`
  or `--only-account`; a Graph carries those choices to its steps.
- Route candidates belong to the executing machine. Their JSON and text omit
  the former local/forwarded `source` field.

The forward database migration preserves existing account/profile identities,
ordered bindings and configured expected logins. Source API compatibility and
database preservation are separate contracts; local tests do not establish that
external crate consumers have migrated.

# LOO-901: `lf account` reads the capacity launches use

Follow-up to LOO-900 / PR #1937 (design at
`7ac0cd900d44225b08f15526cb27028519ce0ca4:scratch/claude-usage-reconciliation.md`).
That Task stays done.

## Problem

Installed `lf 0.15.3+9806b4f3c` refused `lf resume -a claude:fable`: "no
eligible managed claude account; loopflow cooling until 2026-11-17T06:00:05Z
and primary until 2026-11-18T12:00:05Z", while `lf account claude --json`
showed `loopflow` at seven_day 69%, seven_day_fable 16%, both verified live.
The blocker was a stored stream row whose window name is literally
`rejected: seven_day_overage_included` (model `fable`, observed_at 1794572252,
limited_until 1794895205). `primary` had the same window name beside a
genuinely spent polled `seven_day_fable` (100%). #1937 reconciles only the
unprefixed `seven_day_overage_included`.

Three gaps, one cause each:

1. Known historical window names are not recognized as the bucket a poll
   reports, so a refresh neither replaces nor outranks them.
2. `lf account` shows readings and `cooldown_until`, not whether a given model
   can launch; status said "room" while routing said "cooling".
3. `lf account set <login> --clear-cooldown` clears `provider_accounts` only;
   `provider_account_limits.limited_until` keeps blocking launches.

## Direction (Task comment, 2026-11-14, for Jack Heart)

> Broaden the repair so lf account in general checks the capacity that fresh
> launches and resumes use: report effective model-specific eligibility and
> blockers, reconcile known historical formats, and verify against the actual
> installed CLI and persisted historical rows. Also fix --clear-cooldown so its
> documented behavior clears recorded scoped rejection holds consistently;
> currently it only updates provider_accounts and leaves
> provider_account_limits blocking launches. Immediate authorized workaround
> removed exactly two stale rows named rejected: seven_day_overage_included
> from loopflow and primary and incremented capacity_revision on 2026-11-14; a
> fresh poll restores genuine current exhaustion. Do not assume the old live
> rows remain available; their exact fields are in the brief.

Brief constraints: reconcile known formats "without clearing unknown or
paid-credit limits or newer rejections"; "Do not edit live account rows
manually"; "Verify the real installed CLI workflow after delivery".

## Demo

```bash
lf account claude                    # each login: which models launch now, and what holds the rest
lf account next -a claude:fable      # the login a Fable launch or resume takes, or every blocker
lf account set primary@ --clear-cooldown   # names the holds it released; Fable routes again
```

On a store seeded with the incident rows, the first refresh drops the
historical row, `loopflow` reads "fable ready", `primary` reads "fable out
until <reset>", and fresh and resumed `claude:fable` take `loopflow`.

## Findings

Live store, read-only, 2026-11-14 18:49 UTC (`sqlite3 -readonly
~/.lf/loopflow.db`, installed 0.15.3+9806b4f3c). The two prefixed rows are
gone. Claude rows present (`window | used | resets_at | observed_at | source |
model | limited_until`):

```
loopflow | You're out of usage credits. Run /usage-credits to keep using Fable 5.2 or /model to switch models. |100||1794585281|stream|fable|1794586186
loopflow | You've hit your Fable limit · resets Nov 16 at 10pm (America/Los_Angeles). Switch to another model, or manage usage credits at https://claude.ai/settings/usage?from=cc_cli_limit_message, to continue. |100||1794567824|stream|fable|1794568729
loopflow | You've hit your Fable limit · resets Nov 18 at 4am (America/Los_Angeles) |100||1794648420|stream|fable|1794649325
loopflow | You've reached your Fable limit. Run /usage-credits to continue or switch models with /model. |100||1794628635|stream|fable|1794629540
loopflow | session |92|1793401800|1793401455|stream||
loopflow | seven_day |69|1794895199|1794682045|poll||
loopflow | seven_day_fable |16|1794895200|1794682045|poll|fable|
loopflow | weekly |97|1794290400|1794117002|stream||
loopflow | weekly:fable |88|1794290400|1794030785|poll||
primary  | You've reached your Fable limit. Run /usage-credits to continue or switch models with /model. |100||1794395636|stream|fable|1794396541
primary  | session |91|1793401800|1793401254|stream||
primary  | seven_day |65|1795003200|1794682046|poll||
primary  | seven_day_fable |100|1795003200|1794682046|poll|fable|1795003205
primary  | weekly |80|1794398399|1794138907|poll||
primary  | weekly:fable |100|1794398399|1794138907|poll||
```

`provider_accounts`: `primary` has `cooldown_until` NULL with
`cooldown_reason = 'rejected: seven_day_overage_included'`; `loopflow` both
NULL. Every historical row above has an expired or absent hold today; they
never leave because no writer uses those keys again, and `lf account claude`
lists each as a window.

Writers of the historical names:

- `<status>: <rate_limit_type>` as a window name: `record_rate_limit_signal`
  from 90a82635fc (#1828, 2026-11-08) until #1937 pushed
  `window: signal.reason.clone()` when a scoped rejection carried no
  utilization (`git show 9806b4f3cf^:rust/loopflow/src/provider_account.rs`,
  line 801). The decoder then treated any type containing "overage" as credits
  and scoped it to the launched family. Status is one of `rejected`, `blocked`,
  `limited`, `rate_limit_reached` (`harness/claude_decode.rs::rate_limit_signal`).
  The same line wrote `<status>: seven_day_<family>`; none seen live.
- The stated line as a window name: same writer, via `RateLimitSignal::stated`
  (reason = the provider's line, model from `agent::subscription_limit_text`).
- `weekly`, `weekly:<family>` (model NULL), `session` on Claude: poll and
  stream names before #1828 renamed them to `seven_day`, `seven_day_<family>`,
  `five_hour` (`subscription::claude_windows`).
- Current code still writes a non-canonical name on purpose for paid credits
  (`<status>: overage`, "You're out of usage credits…") and for a model outside
  `config::CLAUDE_MODEL_FAMILIES`.

Other facts the design rests on:

- A poll already holds a spent window itself: `subscription::poll_account`
  sets `limited_until = resets_at + RESET_GRACE_SECS` at ≥100%. Genuine
  exhaustion needs no carried-over alias.
- Routing and its error text read `provider_account::account_hold_deadlines`
  (`account_route_eligible`, `account_unavailable_reason`,
  `account_capacity_holds`). `lf account` renders raw `row.windows` and
  `row.cooldown_until` instead (`account_status.rs::row_lines`,
  `render_table`).
- `lf account next` asks with `model = None`
  (`provider_account::inspect_provider_route`, `profile.rs::next_launch`);
  fresh and resumed launches pass their model through
  `ordered_selected_candidates` / `SqliteStore::select_provider_account`.
  LOO-900 left `next` model-unspecified; `-a` is parsed on `lf account` and
  ignored there.
- An older `lf` keeps writing a store a newer one migrated
  (`scripts/check_migrations.py::_check_runtime_sql`), and long-lived
  conversations survive `lf install`. A pre-#1937 process can record a
  historical name after any one-time cleanup.
- `AccountRow` and `NextLaunch` are wire DTOs with fixtures:
  `tests/fixtures/dto/{account_next,auth_status,server_schema}.json`,
  `rust/loopflow/tests/fixtures/dto/server/{account_next,account_report}.json`.
  No Swift type mirrors them today.
- `docs/subscriptions.md` currently says clearing cooldown "retains … every
  observed window" and "reports any window holds that remain" — the behavior
  the direction replaces. Clap has no help text on `--clear-cooldown`.

## Chosen approach

Keep `provider_accounts` and `provider_account_limits` as the source of truth.
No new table, no cache, no migration.

### 1. One name for a known bucket

`provider_account::canonical_claude_window(window, model) -> Option<(String,
Option<String>)>`: the key and scope a poll writes for the bucket a stored
Claude row describes. Everything else returns `None` and keeps its own
identity.

| Stored window | Stored model | Canonical |
| --- | --- | --- |
| `seven_day_<family>` | that family | itself |
| `seven_day_overage_included` | a family | `seven_day_<family>` |
| `<status>: seven_day_overage_included` | a family | `seven_day_<family>` |
| `<status>: seven_day_<family>` | that family | `seven_day_<family>` |
| line where `subscription_limit_text` names the row's family, not credits | a family | `seven_day_<family>` |
| `weekly:<family>` | NULL | `seven_day_<family>`, model = family |
| `weekly` | NULL | `seven_day` |
| `session` | NULL | `five_hour` |

Not recognized, so never merged or cleared by a poll: `<status>: overage`, the
credits line, a model outside the families, a model that disagrees with the
name, anything else.

Three store sites use it and lose their literal `'seven_day_overage_included'`:

- **Read** (`reconcile_legacy_account_windows`, called by
  `provider_account_limits` and `select_provider_account`): group Claude rows
  by `(account, canonical)`, keep the existing rank (newest `observed_at`, then
  later hold, then higher use, then the canonical row), and return the winner
  under its canonical window and model. Every reader — routing, urgency,
  `lf account`, `LaunchCapacity` — sees one row per bucket, named as a poll
  names it. A historical row newer than the last poll still wins and still
  holds.
- **Refresh** (`reconcile_provider_account_capacity`): for each polled window,
  load the account's rows, take the prior hold from every row of the same
  canonical bucket, and delete those rows by their stored keys before the
  upsert. Replaces the two SQL statements that name the alias. Runs inside the
  existing revision check, so a rejection recorded during the poll still
  supersedes it.
- **Recovery cause**: `cooldown_reason`'s suffix goes through the same
  function before it is compared with a polled window.

The classifier is the only place historical names appear; its comment names the
writers above. It goes when no pre-#1937 `lf` can write this store.

Rejected: a one-time data migration that rewrites the rows and lets the read
reconciliation be deleted. It is smaller on paper, but a still-running older
`lf` writes the old name again afterwards and nothing would ever reconcile it —
the incident again. A migration plus the classifier is two representations.

### 2. `lf account` reports what launches

`AccountRow` gains one required field, a list of what the login can launch,
computed by the functions routing uses (`account_route_eligible`,
`account_held_until`, and `identity::check_account_identity`):

```
launches: [{ model: string|null, eligible: bool, out_until: int|null, blockers: [string] }]
```

Managed Claude rows carry one entry per `CLAUDE_MODEL_FAMILIES` plus
`model: null` (a launch whose model the provider picks, held by every scope).
Managed Codex rows carry the `null` entry. Local rows carry an empty list.
Cached identity must still match the current credential, as in route inspection.
`blockers` are the strings `account_unavailable_reason` builds, split per
cause, so the report and the launch error cannot disagree.

Text: `lf account <login|provider>` adds one line per login, e.g.
`launches: opus, sonnet, haiku · fable out until Tue 4:00am`; the table's
"`<model>` out until" lines and the detail view's "held … until" read this
list, not raw windows. `cooldown_until` keeps meaning account-wide only.

`lf account next -a <harness>:<model>` asks for that model:
`inspect_provider_route` takes the model the way launches pass it, and
`NextLaunch` gains required nullable `model`. Without `-a`, `next` stays
model-unspecified and conservative. `-a claude:fable` leaves the Codex row
unspecified.

### 3. `--clear-cooldown` releases every recorded hold

`SqliteStore::clear_provider_account_cooldown` clears, in one transaction,
`cooldown_until`/`cooldown_reason` and `limited_until` on every limit row of
the account, and advances `capacity_revision`. Readings (`used_percent`,
`resets_at`, `observed_at`) stay. The message names what was released:
"Cleared Claude login '…' account-wide cooldown and holds for fable until …",
or "… has no active cooldown or window hold". The "observed windows still hold
it" branch goes: nothing holds after a clear.

Clap help and `docs/subscriptions.md` say the same thing: it releases recorded
holds, keeps readings, does not reset provider quota, and the next refresh or
rejection that finds a window spent holds it again.

## Absent and error states

- No limit rows, or a login with no reading: every `launches` entry eligible
  unless the credential or routing state blocks it; blockers say which.
- Refresh fails, is superseded, or returns no window of a bucket: historical
  rows of that bucket stay and keep whatever hold they have; the read side
  still presents one row per bucket.
- Reset time omitted by a poll at 100%: hold carried from the bucket's prior
  rows, as today.
- Unknown `-a` model on `next`: scoped limits match by family substring
  (`limit_affects`), as launches do.

## Exclusions

- Expired paid-credit and unrecognized rows stay listed; nothing here deletes a
  row a poll did not supersede.
- An account-wide cooldown whose recorded cause names no bucket keeps LOO-900's
  rule: held until expiry or a specific recovery.
- No change to `limit_affects(None)`, explicit `--account` routing, Codex
  window names, or the revision mechanism.
- No live-row edits; the live historical rows clear through an ordinary
  refresh after install.

## Delete — do not maintain

- `'seven_day_overage_included'` literals in `store/sqlite.rs`
  (`reconcile_legacy_account_windows` key and rank, the prior-hold `SELECT`,
  the alias `DELETE`) and the SQL-string assertions in
  `claude_usage_reconciliation_legacy_precedence_and_indexed_lookups` that pin
  those statements; the precedence and query-plan behavior they cover moves to
  the classifier-based tests.
- `clear_account_cooldown`'s "observed windows still hold it" message and the
  half of `claude_usage_reconciliation_explains_overlapping_and_cleared_holds`
  that expects holds to remain after a clear.
- `row_lines` / `render_table` reading `window.limited_until` directly.
- The docs sentence "Clearing cooldown retains … every observed window. The
  command reports any window holds that remain".

Survives: revision ordering, unknown and paid-credit retention, bucket scope
outranking reported model, missing-reset deadline, the released-schema upgrade
test.

## Forbidden outcomes

- A second classifier (SQL `LIKE` beside the Rust one), or a migration beside
  runtime reconciliation.
- `lf account` computing eligibility with its own predicate.
- A `--clear-cooldown` that needs a second flag to clear window holds.
- DTO defaults for the new fields.

## Implementation and remaining checks

All three slices are implemented: historical bucket reconciliation, transactional
hold clearing, model-specific account reports and `next -a`. The shared
`account.next` read carries a required nullable `agent`; Swift encodes it and
its CLI argv too. No schema migration or live-row edits.

The clear message uses the common hold formatter: `Cleared Claude login '…'
holds account-wide until …; for fable until …`. The docs follow that format.
A paused or signed-out login still cannot route after clearing capacity holds.

Remaining: gate's boundary checks and the installed
workflow below after delivery. Historical writer retirement still needs a
separate decision before removing the classifier.

## Next implement direction (concept review, 2026-11-14)

The three slices hold up: one classifier, one clear, one eligibility check.
What's left is in how the answer is said. No code changed in the review.

1. **One word for "can't launch until": `out`.** `AccountRow.held` already
   means "pinned by `lf account use`", and the table prints that mark as
   `held` on the line above `launches:`. A second `held_until` on the same row
   means something else. Rename `launches[].held_until` to `out_until`, the
   word the text already prints ("fable out until Wed 04:00"). The clear
   command talks about cooldowns, the word in its flag and in the blockers
   ("cooling for fable until …"):
   - `Cleared cooldowns on Claude login '…': account-wide until …; for fable until …`
   - `Claude login '…' has no active cooldown`
   - docs: "`--clear-cooldown` releases every cooldown recorded against the
     login: the account-wide one and each model's."
   Prose that says a limit "holds back" a model stays; only the field and the
   clear wording change. Fixtures, `server_schema.json` and the docs' `jq`
   example follow.

2. **`lf account` (the table) lists only what is out.** It now prints
   `launches: fable, opus, sonnet, haiku` under every healthy login, doubling
   the table. Before this change a second line appeared only for a login with
   something out, and the docs still say "one row per login, then a line per
   thing to fix". The table prints the line only when an entry is not
   eligible, and only those entries (`fable out until Wed 04:00`). The full
   `launches:` line belongs to `lf account <login|provider>`.

3. **A login-wide blocker is said once.** A signed-out or paused login reads
   `fable: credential is missing · opus: credential is missing · sonnet: … ·
   haiku: …`. When every entry is blocked by the same non-capacity blocker,
   the line is `launches: none · credential is missing` (or `routing is
   explicit_only`), followed by any per-model `out until`. A Codex login reads
   `launches: ready` or `launches: out until …`, not "launch out until".

4. **`eligible` is `blockers.is_empty()`.** Credential, routing, capacity and
   identity are the whole of both, so the two cannot differ today; compute
   `eligible` from the blocker list built in `provider_account` so they cannot
   differ later, and drop the unreachable "is unavailable" fallback in
   `account_unavailable_reason` if nothing else reaches it. A test asserts
   `account_route_eligible == account_launch_blockers(..).is_empty()` across
   missing credential, paused, account-wide and model cooldowns.

Left as is: `launch_summary` separating capacity blockers by their "cooling "
prefix is string-matching its own output; acceptable while `out_until` carries
the time, and gone if blockers ever become structured.

Not decided by this design, taken as assumptions in `questions.md`: whether
plain `lf account next` should check the configured default agent's model
(LOO-900 kept it model-unspecified), and whether `--clear-cooldown` should
spare account-wide polled windows.

## Acceptance check at gate

Tests seed windows relative to the test clock; the incident's absolute
deadlines pass on 2026-11-17. The window string is exact.

- `cargo test -p loopflow --lib claude_capacity_` and the existing handler fixture cover, through
  the account handlers with the fixture usage server (pattern:
  `claude_usage_reconciliation_account_handlers_refresh_both_logins`):
  - Store holds `rejected: seven_day_overage_included`, model `fable`, 100%,
    live hold, on both logins. Before refresh, `launches` reports Fable held on
    both and `next` for `claude:fable` is blocked with both deadlines.
  - Refresh returns 69%/16% for one login and 65%/100% for the other. After:
    the prefixed row is gone from both; first login `fable` eligible; second
    held until its Fable reset plus grace; Sonnet eligible on both; fresh and
    resumed `claude:fable` select the first login; `claude:sonnet` selects
    either. JSON `launches` and `next -a claude:fable` agree with selection.
  - Each table row above reconciles; `<status>: overage`, the credits line and
    a non-family model survive a refresh with their holds.
  - A historical row observed after the canonical reading still holds; a
    rejection recorded during the poll supersedes the refresh.
  - `--clear-cooldown` on the held login: account cooldown and every
    `limited_until` NULL, readings unchanged, revision advanced, Fable routes
    there, message names the released hold.
- `cargo test -p loopflow --lib claude_usage_reconciliation_` retains existing coverage.
- DTO fixture tests for `account_report`, `account_next`, `auth_status`,
  `server_schema` — include the new required fields.
- `EXPLAIN QUERY PLAN` for the per-account limits read in refresh uses the
  primary key prefix (existing 10,000-account fixture).
- `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`.

After delivery, on the installed build (verify step; read-only except an
ordinary refresh):

- `lf account claude --json` — `windows` for both logins name only
  `five_hour`/`seven_day`/`seven_day_<family>` plus the one credits line;
  `launches` matches `lf account next -a claude:fable --json`.
- `sqlite3 -readonly ~/.lf/loopflow.db "SELECT account_id, window FROM
  provider_account_limits WHERE provider='claude'"` — the stated Fable lines,
  `weekly`, `weekly:fable` and `session` rows listed under Findings are gone
  where the refresh returned their bucket.
- `lf resume -a claude:fable` on a Fable conversation launches when
  `launches` says a login is eligible, and otherwise fails naming the same
  blockers.
- `--clear-cooldown` is exercised on a copy under an isolated `LF_HOME`, not on
  the live logins.

## Done when

- A store holding any recognized historical row routes Fable after one
  refresh shows room, and keeps a genuinely spent login held.
- `lf account` and `lf account next -a claude:<model>` say, per model, what a
  fresh launch or resume would be told.
- `--clear-cooldown` leaves no recorded hold, and its help and docs say so.
- Paid-credit, unrecognized and newer rejections are untouched by a refresh.

## Local checks (2026-11-14)

- `cargo build -p loopflow --bin lf` — passed; isolated candidate `account next -a claude:fable --json` reports Claude model `fable` and Codex model null.
- `cargo nextest run -p loopflow --lib -E 'test(claude_capacity_) | test(claude_usage_reconciliation_) | test(lf::commands::account::account_status::tests::) | test(lf::commands::profile::tests::) | test(engine::account::tests::) | test(server::schema::tests::) | test(store::query_plans::tests::)'` — 43 passed.
- `cargo clippy --all-targets -- -D warnings` — passed.
- `cargo fmt --check` and `git diff --check` — passed.
- Gate: `documented_commands::`, `served_reads::account_holds_and_rotation_use_the_same_machine_state_as_the_cli`, remaining Swift/iOS and website boundaries; CI owns the suites. Installed workflow stays with verify after delivery.
- `scripts/test_desktop.sh --filter ServerProtocolTests` — 20 passed, including non-null model argv and request encoding.
- `cargo nextest run -p loopflow --lib -E 'test(lf::tests::) | test(lf::commands::read::tests::desktop_encodings_name_their_reads) | test(lf::commands::account::account_status::tests::account_report_fixture_preserves_missingness_and_readable_widths)'` — 46 passed, including the corrected fixture and non-null model encoding.

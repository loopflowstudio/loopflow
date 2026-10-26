# LOO-602 — What Loopflow uniquely enables, and the tracking it needs

Status: **draft for Jack's decisions** (2026-10-26). Jack Heart requested the
review from LOO-598. Nothing below is accepted until Jack picks among the
decisions in §7; follow-up Tasks are filed only for accepted ones.

## Problem

Loopflow's store has been shrunk three times: LOO-390 (Oct 7, 2.6 GiB, one row
per streamed token), LOO-570 (#1666, merged Oct 24, in unpublished v0.14.16),
and now this. Each round trims one payload shape. The store regrows from the
next shape, because nothing says what the store is *for*. This design names the
capabilities that only Loopflow provides, derives the facts they need, and
gives everything else to the harness or to files that rotate.

## Live measurements (Jack's machine, installed lf 0.14.15, 2026-10-26)

`~/.lf` is **76.2 GB against a 20 GB `disk_budget`** (`lf disk`).

| Area | Size | Notes |
|---|---|---|
| `copies/` (3 dev-draft store copies) | 37.8 GB | each a full copy of a 9.5 GB pre-prune store |
| `accounts/` (per-account provider homes) | 18.2 GB | codex 17 GB (`engineering/sessions` 6.2 GB, `packages` 1.9 GB, `logs_2.sqlite` 1.4 GB, `thread_history_1.sqlite` 1.4 GB); claude 0.9 GB. "never rotated" |
| `loopflow.db` | 9.6 GB | + 64 MB WAL |
| backups | 9.1 GB | `loopflow.db.backup-0.14.12.001_release` |
| `runtime/exec-processes-archive-20261021` | 906 MB | unknown to rotation |
| `runs/` (captures, verbatim `events.jsonl`) | 74 MB | rotates |

Outside `~/.lf`: `~/.claude/projects` 2.9 GB, `~/.codex/sessions` 7.9 GB.

`loopflow.db` by table (dbstat): `session_events` 8.0 GB / 3.45M rows; its five
indexes 0.9 GB; `processes` 434 MB / 1.05M rows plus 0.31 GB of indexes.
Everything else (Tasks, Waves, planning, graphs) totals **< 60 MB**.

`session_events` payload bytes by kind:

| kind / evidence type | rows | MB | avg B | what it is |
|---|---|---|---|---|
| observed `provider_output` | 1.31M | 3,407 | 2,737 | verbatim provider stream line |
| observed `conversation` | 887k | 2,682 | 3,172 | same content, normalized |
| observed `manifest.json` | 6.3k | 431 | 71,463 | launch manifest incl. `exec.system_prompt`/`task_prompt` |
| observed `user_input` | 9.0k | 238 | 27,904 | full submitted prompt text |
| observed `activity` | 149k | 119 | 840 | stream activity readings |
| observed `usage` | 157k | 91 | 608 | per-checkpoint usage (stream copy) |
| `usage` (native receipt) | 111k | 36 | 339 | per-turn usage (native copy) |
| everything else (started/completed/output/captured, attempts, accounts, sessions, terminal) | ~60k | ~20 | — | **the facts** |

So ~6.1 GB (87%) is transcript content the harness already stores; ~0.7 GB is
prompts; ~20 MB is the facts every reader actually needs.

**Usage is recorded twice.** Of 599 Sessions with usage in the last 3 days, 588
have both native `usage` receipts (written by `harness/*_history.rs`) and
`events.jsonl:usage` observations (written by the recorder). Both survive the
LOO-570 prune (`RETAINED` keeps `type='usage'`).

`processes` by command: `task file` 379k, `task diff` 165k, `session list` 102k,
`monitor ps` 98k, `__context-block` 51k, `wave list` 20k, `roadmap --all` 20k.
~85% are Desktop polls and provider hooks. `LfCli::records_process`
(`rust/loopflow/src/lf/mod.rs:217`) now skips `task file/diff`, `ContextBlock`
and `__*`; in the last 2 days the mix is `__context-block --moment` 3.2k (a
released relay still records it), `task status` 1.1k, `session list` 156.
Growth now: ~27k events / 68 MB per day before the prune.

LOO-570's clone probe (#1666): the prune takes this store from 9.8 GB to 550 MB.
It has not run here: v0.14.16 is merged but unpublished (latest release
v0.14.15).

Incidental: `lf account --cached` lists `codex fixture-cf433b35f6f1 ·
fixture@example.com` — a test wrote a managed login into the live home.

## 1. What only Loopflow can do

Harnesses each own one conversation on one account. Loopflow's distinct value is
everything *across* them. Grouped by the questions Jack asked:

**Monitoring.** One live view of every agent on the Machine, any harness, tied to
the Task/Wave it serves: what is running, waiting for a person, stalled (silence
> 600 s), or finished; which Graph step it is; its process tree (`lf monitor`,
`lf ps`, Desktop Work). No harness can see its siblings or know that a Session
belongs to LOO-602.

**Account management.** Several logins per provider; routing a launch to the
account with headroom; capacity readings from provider usage endpoints
(`lf account`, `subscription.rs`); resume on another account (LOO-324). Harnesses
know one login at a time.

**Telemetry and cost.** Usage attributed to Task, Project, Wave, Graph step and
skill, across providers and accounts, over months (`lf history usage --task …
--weekly`, `--context` for per-source prompt cost, `--binds`). Harness
transcripts have tokens per message but no Work attribution, and Claude deletes
transcripts after `cleanupPeriodDays` (default 30) — so a harness cannot answer
"what did the Infrastructure wave cost in September".

**Execution continuity.** Durable input that survives a crash (captured input),
replay of an exact launch (`lf history replay`), Graph results read from step
exits, steering into a running turn. Harnesses resume a thread; they do not know
a Graph or a Task.

Everything else a person reads about a conversation — the turns, tool calls,
text — is the harness's own product and its own file.

## 2. The tracking those capabilities need

| Capability | Fact needed | Durable? | Size per unit |
|---|---|---|---|
| Live state / availability | turn started, turn completed (status), last activity time, silence intervals | started/completed/silences: yes; activity: **live only** (latest per Session) | ~150 B/turn |
| Work attribution | Session → Task/Wave/Graph step; Process tree; captured input id | yes | ~300 B/input |
| Account routing & capacity | account selected per attempt; provider capacity snapshot | selection yes; capacity: latest per account | ~250 B/attempt |
| Cost | **one usage row per native turn** (tokens by class, model, cost if reported) | yes, forever | ~340 B/turn |
| Final answer for Graph routing | final output text of a deciding step | yes (`output` kind) | ~1 KB/step |
| Native locator | provider, account home, native session id | yes | ~250 B/Session |
| Replay | manifest + submitted prompt | file in `runs/`, rotates | 15–70 KB/input |
| Transcript content | — | **harness's file** | 0 in store |

At today's rate (~6.3k inputs, ~111k native turns over ~3 weeks of live history)
the durable facts are on the order of **50–60 MB per year**, with Processes kept
only while referenced. That is the default footprint.

## 3. Default small, opt-in larger

**Default (the store holds facts only, by construction).**
- `session_events` admits the fact kinds above and nothing else. The
  `observed` kind with a free JSON payload is how transcript content kept
  re-entering; the default replaces "filter after write" (LOO-390 coalescing,
  LOO-570 `retained_in_history` + prune) with "cannot write". Concretely: the
  recorder stops writing `events.jsonl`-sourced rows to SQLite at all; native
  receipts (`started/usage/completed/output`) plus typed attempt/account/session
  /silence facts are the only Session history rows.
- Activity is a live reading, not history: one row per Session updated in place
  (`session_activity` already exists for this), not 149k appended rows.
- Inspection Processes: today a denylist (`records_process`) plus a 7-day
  expiry. Default: a Process row is written for every lf process (the live tree
  needs it) and **deleted at exit** unless something references it (a child,
  a Session event, a Graph step) or it was agent-issued. The denylist goes.
- Prompts and manifests: files in `runs/<capture>/`, rotated with captures;
  the store keeps the capture id only (LOO-570 already nulls `exec`).
- `disk_budget` covers what Loopflow owns. Provider homes under
  `~/.lf/accounts/` are reported separately with the provider's own retention
  knob, not counted against a budget lf cannot meet (today 18 GB of the 20 GB
  budget is un-rotatable, so rotation can never succeed).

**Opt-in (config grows the footprint for more power).** One setting,
`capture.stream: true` (Machine config), keeps the verbatim provider stream in
`runs/<capture>/events.jsonl` for harness debugging and `lf history show
--events`. Off by default; rotated by `disk_budget` either way. Nothing opt-in
ever lands in SQLite.

## 4. What mirrors the harness, and what to defer

| Loopflow copy | Harness original | Decision |
|---|---|---|
| `provider_output` + `conversation` rows (6.1 GB) | Claude `projects/<cwd>/<id>.jsonl`; Codex `sessions/YYYY/MM/DD/rollout-*.jsonl`; OpenCode storage; Pi sessions | **Delete from store.** LOO-570 prune already removes; make the recorder unable to write them (§3). |
| `runs/*/events.jsonl` verbatim stream | same | **Opt-in** (`capture.stream`). |
| `user_input` rows (238 MB) | first user message in the native transcript | **Delete from store**; replay reads `runs/<capture>/` prompt file. |
| observed `usage` (stream copy) | per-message usage in native transcript | **Delete** — native receipt `usage` is the single source. |
| native `usage` receipts | same, but expire after 30 days in Claude | **Keep** — attribution + longevity are Loopflow's value. |
| per-account Codex homes (`packages/` 1.9 GB per home, `sessions/`) | `~/.codex` | Keep isolation; LOO-324/LOO-540 own sharing sessions across homes. Report sizes in `lf disk`, never delete. |

**Unified interface over deferred storage.** `lf history show <session>` (and
Desktop's conversation view) renders turns by reading the native transcript
through the Session's stored locator, one small reader per harness
(`harness/<h>_transcript.rs`, `fn turns(locator) -> Option<Vec<Turn>>`), mapping
into the existing `ConversationItem` types. When the native file is gone (Claude
30-day cleanup, a deleted Codex home), history shows the retained facts — turn
times, status, usage, final answer — and says the transcript expired at the
provider. That is truthful absence, not an error. LOO-324's native-history
discovery is the same lookup; build it once.

## 5. The translational layer, and how to simplify it

Today one native line passes through three decoders and two durable writers:

```
public surface (lf CLI, --json DTOs, Desktop via lf subprocess, Discord)
  → ops/ (task, graph) → agent/ (AgentConfig, capture, launch)
  → Harness trait (harness/mod.rs:251) × 4 impls, 15k lines
      per native line, e.g. harness/claude.rs:239-269:
        rate_limit_signal(line)            → account capacity
        <h>_history::record(line)          → session_events native receipts
            + activity_tracker              → session_activity
        <h>_mapping::process_line(line)    → ConversationEvent (chat/types.rs)
            → session_record recorder      → events.jsonl + observed rows (filtered by retained_in_history)
```

Per harness: `X.rs` (driver), `X_mapping.rs`, `X_history.rs`, plus shared
`activity_tracker.rs` — 4,321 lines of decoding for 4 harnesses, each parsing
the same JSON. The two durable outputs disagree on shape (native receipts keyed
by provider turn; observations keyed by stream seq) and duplicate usage.

**Target: one decoder, one writer.**

```
Harness driver ─native line─▶ <h>_decode(line) -> Vec<ProviderFact>
                                   │                     (TurnStarted, TurnCompleted{status},
                                   │                      Output{text}, Usage, Activity,
                                   │                      SessionObserved, AccountSelected,
                                   │                      RateLimit)
                                   ├─▶ FactWriter  → session_events (facts only), session_activity (latest)
                                   └─▶ live subscribers (ConversationEvent for Desktop/Discord stream; not stored)
```

- `ProviderFact` is the single translation boundary. Everything durable,
  every availability decision, and every cost number derive from it.
- `ConversationEvent` survives only as the live-display projection (and the
  transcript reader's output type), never persisted.
- The recorder (`SessionRecorder`, `retained_in_history`, the `events.jsonl`
  → SQLite path) is deleted; `capture.stream` is a raw tee to a file.
- The CLI/DTO surface does not change: `lf history`, `lf history usage`,
  `lf monitor`, Desktop's `--json` reads keep their wire types, now fed by facts
  plus the native transcript reader.

## 6. Demo

On Jack's machine, after the change installs:

```
lf disk                      # Home ~/.lf  ~2 GB of 20 GB budget; provider homes 18 GB (reported, not budgeted)
sqlite3 ~/.lf/loopflow.db "select sum(pgsize) from dbstat"   # < 200 MB, growing < 1 MB/day
lf history usage --wave infrastructure --weekly   # same totals as before for weeks since 2026-09-30
lf history show <today's session>                 # turns rendered from ~/.claude/.../<id>.jsonl
lf history show <a 45-day-old claude session>     # facts + "transcript expired at provider"
```

## 7. Decisions for Jack (draft; nothing filed until chosen)

1. **Facts-only store by construction** (§3): recorder stops writing stream
   observations; delete `SessionRecorder` persistence, `retained_in_history`,
   and — once stores are pruned — the prune's capture pass (`5b0109`) and
   `RETAINED` SQL. *Recommended.*
2. **Native usage receipt is the one usage record**; drop stream `usage`
   observations and their `store_revision_usage_insert` branch. *Recommended*;
   needs a reader trace first (which of `context_usage.rs`, `lf history usage`
   read which copy).
3. **Verbatim stream opt-in** (`capture.stream`, default off). Alternative:
   keep it default-on since it rotates and is only 74 MB here. The cost of
   default-on is a second transcript format to maintain, not bytes.
4. **Transcript rendering from native files** with truthful expiry (§4). This
   changes what `lf history show` can show for old Sessions. Alternative:
   keep a compact per-turn summary text in the store for longevity.
5. **Process rows deleted at exit unless referenced** (§3), replacing the
   `records_process` denylist and 7-day inspection expiry.
6. **`disk_budget` excludes provider homes** and reports them separately; rotate
   `runtime/exec-processes-archive-*` and pre-prune backups/copies.
7. **One decoder per harness → `ProviderFact`** (§5), merging
   `_history`, `_mapping` and `activity_tracker`. Largest change; the other
   decisions do not depend on it, but it removes the double-write class for good.

Immediate, no decision needed: publish v0.14.16 so the LOO-570 prune runs here,
then re-measure (store, copies, backups).

## Follow-up Tasks (to file per accepted decision)

| Decision | Task sketch | Existing Task it absorbs/relates |
|---|---|---|
| — | Publish v0.14.16; measure post-prune store, first `lf install` time, copies | release-run; #1666 follow-up |
| 1, 2 | Store holds Session facts only; one usage record | `5b0109` (delete prune capture pass), LOO-459 (history retention) |
| 3, 4 | Read conversations from harness transcripts; `capture.stream` opt-in | LOO-324 (native discovery across homes) |
| 5 | Process rows live while running, kept only when referenced | — |
| 6 | `lf disk` budget covers lf-owned data; provider homes reported | LOO-460, LOO-448 |
| 7 | One decoder per harness emitting `ProviderFact` | — |
| — | Find and fix the test that wrote `fixture@example.com` into the live account store | — |

## Delete — do not maintain (when decisions 1, 2, 5, 7 are accepted)

- `session_record.rs`: `SessionRecorder` stream persistence, `retained_in_history`,
  stream-to-SQLite observation writes.
- `store/sqlite/retention.rs`: capture pass, `RETAINED`, `compact_usage` once
  converted (`5b0109`).
- `session_events` observed `usage`/`activity`/`conversation`/`provider_output`/
  `user_input` row shapes and the trigger branches that classify them
  (`store_revision_session_events_*` `events.jsonl` predicates, `store_revision_usage_insert` observed branch).
- `LfCli::records_process` denylist and `PROCESS_RETENTION_SECONDS` expiry.
- `harness/*_history.rs`, `harness/*_mapping.rs`, `harness/activity_tracker.rs`
  merged into `harness/*_decode.rs` (decision 7).

## Forbidden outcomes

- A second transcript store in any form (SQLite rows, copied JSONL, reconstructed
  context) — Jack rejected this for LOO-324 on 2026-10-04.
- A filter that runs after write as the mechanism keeping the store small.
- Two usage representations, or a reader that sums both.
- Budget numbers that include data lf cannot rotate.

## Risks

- **Native transcript formats change** without notice (Claude/Codex ship weekly).
  The transcript reader is display-only; facts come from the live stream, so a
  reader break degrades `history show`, never cost or Graph routing.
- **Claude 30-day cleanup** removes content for old Sessions. Accepted by
  decision 4, or mitigated by its alternative.
- **Older relays keep writing** old shapes until restarted (seen today:
  `__context-block` rows from a released relay). The prune covers the overlap
  until `5b0109`.
- Desktop reads that depend on stream observations (if any) must move to facts
  in the same change; trace before deleting.

## Done when

- `lf disk` on Jack's machine reports lf-owned data within `disk_budget`, with
  provider homes listed separately.
- `loopflow.db` < 200 MB and growing < 1 MB/day under normal use, measured over 7 days.
- `lf history usage --weekly` totals match pre-change totals for every week since 2026-09-30.
- `lf history show` renders a live Claude and a live Codex Session from native
  files; an expired one shows facts and states the expiry.

## Acceptance at gate (headless)

With inherited `LF_*` unset and `< /dev/null`:
- `cargo test -p loopflow --lib session_record session_events retention` — facts-only
  writer rejects/never emits stream rows; one usage row per native turn.
- `cargo test -p loopflow --test integration history` — `history usage` totals
  unchanged on the fixture; `history show` renders from a fixture native
  transcript and reports expiry when it is missing.
- Clone probe: `cp -c ~/.lf/loopflow.db` to a temp home, run the branch build's
  prune + one recorded headless session; `select kind, count(*) from session_events`
  shows no stream observation kinds for the new capture.

Check: design only, no build/test run (2026-10-26).

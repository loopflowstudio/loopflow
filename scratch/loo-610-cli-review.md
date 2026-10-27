# LOO-610 — The lf CLI, top down

Status: **decided slice implemented** (2026-10-26): items 1–7 and the
Task-first README are on the branch, merged with main `df6b79d5ea` (#1704,
#1706, #1707), and `machine use`'s boolean `--repo` is deleted. Remaining:
gate, publish.
Deviations in `scratch/questions.md`. Sections 1–8 are the review as proposed;
a later decision wins where they differ. Source: `docs/lf-reference.md` at
main `1b137b8f2d`, LOO-601 (hidden `__` commands) and LOO-602's decisions.

## Direction (Jack Heart, 2026-10-26)

- **No experimental tier.** A command is promised or deleted. Code is easy to
  bring back.
- **1.0 promises what we've been building:** some telemetry, a controllable
  GUI, Task management that just works, Session management.
- Organize the API by the product's pillars (README: Work, Context,
  Orchestration, Sessions, Skill Graphs) and see what goes where. The right
  set is open: three, four or five.

- **Surfaces (Jack Heart, 2026-10-26).** An API that works wherever lf is
  invoked is the helpful part: Loopflow can be told "call this exact API".
  Translating every Desktop verb into Herdr/tmux/cmux/Rex calls is not
  necessary. Rex (and any programmable host) users calling their host's API
  directly is great; lf does not compete with it.

- **Existing tools first (Jack Heart, 2026-10-26).** If people can do it with
  existing tools and lf adds nothing on top, delete it from the API. Scheduled
  jobs live in people's own cron/launchd running an ordinary lf command:
  `wave cron` is deleted (D4 decided). A record of crons Loopflow suggested may
  come later; not now. Today's two installed jobs (`release-run`,
  `telemetry-daily`) become crontab lines.

- **`lf serve`: build it (Jack Heart, 2026-10-26, D6).** Build whatever it
  needs; taking longer is fine. The point is to get it right, so explore
  improvements to the current system rather than only recording a rule.
- **Delivery (D7).** One big design here, chunked into follow-up Tasks. Every
  Task, this one included, lands a substantial piece of real work; avoid
  design-only Tasks.

- **Repo is the Wave of a Task without one (Jack Heart, 2026-10-26).** Not a
  Wave object, which keeps "every Task is in some scope" structural. So `--repo`
  is a scope selector, like `--wave`; `lf repo` as a command grab bag goes.
  - `release` belongs with `pr land` and CI in a software-ops group. Jack is
    unsure whether others release this way; he aspires to run all his projects'
    releases through it.
  - `tokens` → `history`.
  - `connect`, `reteam` → the work model, probably Wave-level.
  - `refresh`, `planning status/key`, `identity`: ask whether the right API
    exists; a special-case command is a way to solve something, not proof it's
    the right way. Probably work configuration.
  - `new-chapter`: a work-model substream; cut it for now.
  - `comment` → Session.
- **Tree by pillar, typed by elision.** Commands already resolve when the owner
  is unambiguous (`lf land` → `lf pr land`), so pillars can be real groups
  (`lf work task …`) at no typing cost.

- **Delivery is Loopflow's vocabulary, not GitHub's (Jack Heart, 2026-10-26).**
  Land is a first-class Loopflow concept. The code host and CI should be
  configurable the way planning (Linear/Git) and harness (Claude/Codex/…) are:
  Loopflow translates external dependencies into an abstract workflow language
  so workflows are written generically. So `pr` commands are not "thin `gh`
  wrappers" to delete: removing them would make lf's shipping instructions say
  `gh`, and a non-GitHub user would have nothing. The "existing tools" rule
  counts provider neutrality as something lf adds when the concept belongs to
  the workflow language (publish, checks, reviews, land, release).
  Today the promise outruns the code: `gh` is called directly from ~20 modules
  (`ops/land.rs`, `ops/pr.rs`, `ops/ci_watch.rs`, `ops/release.rs`,
  `ops/task/lifecycle.rs`, `git/worktrees.rs`, …) with no code-host port. A
  follow-up Task puts the code host behind one port, as planning already is.

## The shape we're aiming for

A recruit (or an agent trained on lf transcripts) should be able to guess the
CLI from the model. Seven nouns, a few verbs, everything else under them:

```
lf <skill|graph> [message]        run work (lf pursue, lf : "fix the build")
lf task …      one outcome: create, run, status, wait, complete
lf wave …      a durable objective and its plan
lf session …   a conversation: list, resume, steer
lf graph …     authored steps and the runs they made
lf pr …        delivery
lf history …   what happened, what it cost
lf machine …   where work runs (local, SSH, rented)
lf account …   provider logins and capacity
lf open        show any Work in your host (Desktop, cmux, herdr, tmux, rex, terminal)
lf ps          what is running right now
lf sync / lf commit / lf wt      git-adjacent verbs
lf help / lf list / lf self      discovery and the install
```

Plus two GUI-API nouns (§4): `lf watch` (live streams) and `lf view`
(Dashboards). Everything else folds in, becomes a skill, is marked experimental,
or is deleted. Root help today lists 26 commands; this lands at **18** plus
`help`, `list` and `:`.

Rules applied throughout:

1. **One verb per meaning.** "resume" currently means both "continue a
   conversation" (`session connect`, alias `resume`) and "clear a Task pause"
   (`task resume`). "comment" and "steer" name the same act across repo/wave/
   task/session. Each verb gets one meaning.
2. **Owners match the model.** Planning lives under `wave`/`task`, compute under
   `machine`, conversation under `session`, the past under `history`.
3. **The CLI is the API.** Desktop, a custom tmux UI, an SSH caller and an
   opt-in server all speak the same thing: an lf argv in, one JSON document (or
   NDJSON stream) out. No second protocol.
4. **Promise only what the infrastructure can keep.** See §5.

## 1. Top-level dispositions

| Command | Disposition | Reason |
|---|---|---|
| `task` | **keep**, slim (§2) | Core noun. 37 subcommands → ~22. |
| `wave` | **keep**, slim (§2) | Core noun. Planning plumbing moves to a `plan` group; `cron` leaves. |
| `session` | **keep**, reshape (§3) | Core noun; resume becomes one verb. |
| `graph` | **keep** | Core noun. `list --processes`/`show --processes` is the run inspector. |
| `skill` | **keep** | `lf skill NAME` disambiguates; `customize` is the authoring door. |
| `pr` | **keep**, 10 → 7 (§2) | Delivery. |
| `history` | **keep**, absorbs `session history` (§5) | The past and its cost. |
| `account` | **keep**, fix positional shape | `lf account <provider> use <email>` reads backwards; make it `lf account use <provider> <email>` like its siblings. |
| `machine` | **keep**, absorbs `pool` + `ondemand` (§6) | One noun for "where work runs". `pool`/`ondemand` are unreleased (not in v0.14.15), so folding costs nothing. |
| `open` | **keep** | Already connective: `--host auto|desktop|cmux|herdr|tmux|rex|terminal`. The model for the rest. |
| `sync`, `commit`, `wt` | **keep** | Essential git-adjacent verbs agents use constantly. |
| `help`, `list` | **keep** | Discovery. |
| `self` | **keep** | `doctor`, `install`. Absorbs hidden `install recover-switch/preflight/advance-switch/promote/rollback` as one hidden `self install --continue <receipt>` (LOO-601 left these here). |
| `:` | **keep** | Inline prompt. Cheap, memorable. |
| `monitor` | **split** → `lf ps` (person) + `lf watch` (GUI streams) | Bare `monitor`, `active`, `ps`, `top` are four views of "what's running" → `lf ps` (`--watch` refreshes like `top`). `monitor work --watch` is Desktop's stream → `lf watch work` (§4). `prune` → `lf self doctor --repair`. |
| `desktop` | **reshape** (§4) | Two different APIs share the name. Pane control keeps `lf desktop`; Dashboard data moves to the GUI API. |
| `repo` | **dissolved** (scope item 2) | Grab bag of 10; `repo` is a scope selector, not a group. |
| `disk` | **merge** → `lf self disk` | Maintenance, not a concept. `rotate` dies with `disk_budget` if LOO-602 decision 6 lands as leaning. |
| `portfolio` | **merge** → `lf config repos` | It is configuration (which repos Desktop shows). `import` is a one-time migration: delete. |
| `config` | **keep as group**, grows `repos` | Today a one-command group (`config user`). |
| `context` | **merge** → `lf help context` / `lf --context` | A diagnostic; the root `--context` preview already exists. |
| `scratch` | **delete** from public surface | Only `clear`, which `pr submit`/`pr land` already run. Keep the function, drop the command, or keep hidden for skills that call it. |
| `clone` | **delete** from public surface → `scripts/` or a dev-only build feature | Development tool for Loopflow itself; no recruit needs it. |
| `discord` | **experimental** (§6) | A server. Fold into the opt-in `lf serve` or keep behind `experimental`. |

Result in root help: `task wave session graph skill pr history account machine
open ps watch view sync commit wt self config`, plus `help list :`.

## 2. Inside the big nouns

### `task` (37 → ~22)

| Today | Proposal | Why |
|---|---|---|
| `create edit move comment status list complete reopen abandon delete` | keep | Planning lifecycle. |
| `run wait interrupt` | keep | Execution. |
| `checkout` | keep | Explicit "give me the worktree". |
| `workflow show/move/restart` | keep `show`, `move`; **delete `restart`** | `restart` = `move <issue> start`. |
| `defer` / `resume` | **rename** `resume` (D2) | Main (#1652) renamed `pause` → `defer`; `task resume` still clears it. Open: D2's `hold/release` predates `defer` — LOO-618 picks `defer`/`undefer` or `hold`/`release`. |
| `claim` / `unclaim` | keep (claims are real ownership) | — |
| `sync` | **delete** (automatic on read/write) | Provider sync is plumbing; failures surface in `status`. Keep `--retry` on `status` if needed. |
| `sweep` | → skill (`triage`) | Planning judgment, not a primitive. |
| `location` | merge into `status --json` (`location`, `--peers`) | A field of status. |
| `shell` | **move** to the GUI API | Desktop's remote-shell pane runs `--machine M task shell T`; a person uses `lf --task X` / `lf open --host terminal`. |
| `diff files file save watch-files` | **move** to the GUI API (§4) | Desktop's file editor. Not a person's command. |

### `wave` (19 + cron 9)

- Keep `list status workflow comment rename`. **Merge `show` into `status`**
  (`status --all` for history) — two readers of one plan.
- `edit-plan update-plan new-chapter ensure bind-project` + `repo connect/
  refresh/reteam/new-chapter/identity/planning` → **`lf wave plan …`** (one
  group, about the Linear/Git plan binding). Most are operator plumbing run by
  `start-chapter`/`review-chapter`; mark the group experimental if Jack prefers.
- `wave cron` leaves `wave`: it installs launchd jobs that run any lf command
  on this Machine, which is not a Wave concept (D4 picks the home).
  `disposition` folds into `history`.

### `session`

See §3.

### `pr` (10 → 7)

`open publish submit land abandon checks reviews` keep. `arm` merges into
`land` (`land` already requests auto-merge and returns; `arm` differs only by
not retaining settlement intent — make that `--no-settle` if it is still
needed). `request-review` → `reviews --request <login>`. Main (#1652) deleted
`ci watch` and `task reconcile`; `pr reconcile` is the one settle-once command
and the documented crontab line. `repo ci` → `lf ops ci` (incident report).

## 3. Session resuming — one verb, no questions

**The promise:** `lf resume` gets you back to the conversation you were in.
You don't need to know the harness, whether it crashed, whether a Task owns it,
or whether another terminal has it.

Today that intent is split across `session connect` (alias `resume`,
shortcut `lf connect`), `session attach`, `open`, and the flags `--fresh
--takeover --try --all --prepare --if-free --observe`. Each flag encodes a
question the person shouldn't have to answer.

| Situation | Today | `lf resume` does |
|---|---|---|
| Live, owned by lf, nobody watching | `session attach` | attaches |
| Live, another terminal watching | `attach --if-free` / `--takeover` | attaches as second viewer, takes input; old viewer sees "taken here" |
| Process died (crash/restart) | `connect` (or `--all`) | resumes provider conversation on the right harness and account |
| Started outside lf (raw `claude`) | `connect <provider-id>` / LOO-424 | resumes by provider id; binds a Session |
| No id given | `connect` → scope primary | the last Session you were in here (cwd → Task → Wave → repo primary) |

Surface after:

```
lf resume [id|task]        # the one verb; `lf session resume` canonical
lf resume --all            # everything a crash interrupted
lf session new [--task]    # today's --fresh
lf open …                  # present it in a host instead of this terminal
lf session list / steer / rename / bind
```

Deleted: `session attach` (resume detects liveness), `session connect`
(renamed), `--try`, `--takeover`, `--if-free` (defaults), `session
observe-status` and `--prepare` (GUI API, §4), `session history` (→ `history
show`).

**Decided (Jack Heart, 2026-10-26, D2):**

- Bare `lf resume` means the last Session *you* were in from here: this
  worktree's most recent, then its Task's primary, then the Wave's, then the
  repository's.
- `lf resume` takes over by default: it stops whatever other client holds the
  conversation and continues it here. No `--takeover`, `--try`, `--if-free`.
- Opt out with a flag (Jack: "--fork or --save or something"; draft name
  `--fork`): leave the other client running and continue in a new conversation
  forked from it.
- `--fresh` → `lf session new [--task|--wave]`; `task resume` (clears
  `defer`) gets a non-`resume` name; `lf connect` stops being a separate name.
- `lf session new` confirmed (Jack Heart): "restart" read as the same Session
  and sat too close to "resume". Help: "Start a new conversation for this
  scope; the previous one stays in history."
- When nothing exists to resume, `lf resume` starts one and says so.
- Revised (Jack Heart): in a Task worktree, bare `lf resume` is the Task's
  conversation. `resume` always continues the same provider thread;
  `lf session new [-a X]` is the restart: a fresh thread seeded with what the
  last one knew, and the way to switch harness. `lf resume` has no `-a`: it
  keeps the same harness and thread. Filed as LOO-618.
- To check in implementation: takeover of a client lf did not start (a raw
  `claude` in another terminal) — lf may only be able to signal its process;
  say what happened either way.

LOO-424 becomes the implementation of row 4 plus "resume from anywhere"
(another Machine via `--machine`). It is absorbed: this review reshapes it into
"`lf resume` meets the table above", not a separate surface.

## 4. Who calls lf: the GUI API

Desktop already proves rule 3. It runs ~75 argv shapes, all through its bundled
`lf`, almost all `--json`, no `__` commands and no timer polling: live state
arrives over three NDJSON streams it holds open (`monitor work --watch`, `task
watch-files`, `session observe-status`). Remote Machines are the same argv with
`--machine M --repository R` prefixed (`RegistryQuery.swift:103`).

Grouped, that set *is* a GUI API — it just isn't named or documented:

| Group | Calls Desktop makes today | GUI API name after |
|---|---|---|
| Live Work | `monitor work --watch` (stdin requests: scope, refresh, checkpoint, taskHistory) | `lf watch work` |
| Terminal status | `session observe-status <sid> --terminal T` | `lf watch session <sid>` |
| Task files | `task diff/files/file/save`, `task watch-files` | `lf task files …` (one group: `ls cat save diff watch`) |
| Conversation launch | 4 flavours of `session connect --prepare` + `session attach --prepare` | `lf resume <id> --prepare` (one shape; §3 removes the flavours) |
| Dashboards | `desktop view/query/pin/unpin/rename/arrange/slots/order/layout` | `lf view …` (it's Work selection, not Desktop control) |
| Reads | `wave list/show/status`, `history`, `usage`, `graph list/show`, `portfolio`, `config user`, `machine id`, `open --link` | unchanged, documented |

**Two directions share `lf desktop` today.** Dashboard/selection data (`view
query pin arrange slots order layout`) is client state any UI could store; the
pane verbs (`text key read hide restore focus shell files graph-log move
resize zoom state`) drive *this* Desktop app from an agent. Proposal: the first
becomes `lf view` (the GUI API's selection model — a tmux UI could render the
same Dashboards); the second keeps `lf desktop` and is experimental for 1.0
(LOO-519 just added it; its nouns already match `--pane`).

`desktop rename` renaming a Task and `desktop order` reordering Tasks duplicate
`task edit --title/--rank` — delete from `desktop`; Desktop calls `task edit`.

**Build-your-own UI.** Document the GUI API as one page: the streams, their
NDJSON shapes (already DTO fixtures under `tests/fixtures/dto/`), and the
mutations. A tmux/herdr/rex UI shells out exactly as Desktop does.

**Inefficient today** (from the inventory):

- `pinTaskLocation` (`RegistryQuery.swift:124`) runs `task location` before
  *and* after every remote-routed call: 3 processes per call. → file.
- Graph inspector: `graph list --processes --for-task`, then one `graph show
  --processes` per entry (N+1). → file; `list --processes` should carry steps.

## 5. Telemetry and promises the infrastructure must keep

LOO-602 settled most of the substance (facts-only store, providers retain text,
one usage record, one decoder). What's left for the API is which commands make
which promises.

**Telemetry API, after:**

```
lf history                       # recent Work, all scopes
lf history show <session|process>    # turns from the provider transcript, or "expired at provider" + facts
lf history usage [--task|--wave|--weekly|--context]   # cost, from facts only
lf history retention             # what each provider still holds, what expires next (LOO-602 §7.4)
lf history replay <run>          # while its capture file exists
```

`session history` (native receipts) is `history show <session> --facts`.
`history export` stays hidden (machine handoff).

### Telemetry as stories (Jack Heart, 2026-10-26)

Jack: picture the website story where someone types four commands, or opens one
Desktop window, and a problem is solved: what they spent across Waves or Tasks,
what their context is made of, tokens churned over time across all their
systems. These are example queries, not a spec. The API should make that family
easy to express. Stay inside what the facts store can structure cheaply; people
get creative from there.

Stories:

```
lf usage --by wave --since 30d              # what each objective cost this month
lf usage --by task --wave infrastructure    # which Tasks were expensive
lf usage --every week                       # token churn over time (sparkline; --json for charts)
lf usage --by skill                         # which steps of my process cost most
lf usage --by harness,model                 # Claude vs Codex, by model
lf usage --by account --every day           # how my subscriptions are spent
lf usage --by source --task LOO-610         # what my context is made of
```

The language is one table: **filter × group × bucket × measure**.

- Filters reuse lf's selection: `--wave --project --task --repo --machine`,
  plus `--since`, `--skill`, `--graph`, `--harness`, `--account`.
- `--by` takes one or more dimensions: wave, project, task, graph, skill,
  session, harness, model, account, repo, machine, source.
- `--every day|week|month` adds a time bucket.
- Measures are fixed columns: input/output/cache tokens, cost (where the
  provider reports it), turns, Sessions, wall time.
- `--json` emits flat rows `{dimensions…, measures…}` for jq, DuckDB or a
  spreadsheet; Desktop's Telemetry window renders the same query.

Every dimension except `source` is an attribution fact joined to one usage row
per native turn (LOO-602 §2): small, cheap SQL. `source` needs LOO-612's
per-request context facts. Today `history usage` lists one row per Session input
with scope filters only; grouping means piping `--json` through jq.

Open:
- **Across all your systems.** Facts are per Machine. Either `--machine` runs the
  query on each (`lf usage --machines all` merges rows), or `machine end`'s
  record copy becomes the general sync. Leaning: merge rows at query time over
  SSH, no central store.
- **Cost for subscriptions.** Many rows show cost `-`. Either measure tokens
  only, or derive an API-equivalent price from a table and label it estimated.
- **Interactive Sessions.** `task-session` rows show no usage in the sample;
  verify they are counted before telling the "what did I spend" story.

### Classification

| Surface | Class | Owner |
|---|---|---|
| `history show --events` (verbatim provider stream) | **Wrong promise** — deleted (#1706) | LOO-613 |
| Unbounded transcript history in Loopflow's store | **Wrong promise** — providers retain; lf reads them | LOO-614 |
| `disk rotate` / `disk_budget` covering provider homes lf can't rotate | **Wrong promise** — budget only what lf owns, or drop it (LOO-602 D6) | this Task (API) + LOO-602 D6 |
| `history replay` of any run, forever | **Wrong promise** — help now says "while this Machine retains its capture" (built) | this Task |
| `session list --limit 0` "complete matching inventory" | promise sound at facts-only scale | — |
| `history usage` / `--context` reading two usage copies + raw stream | **Inefficient today** | LOO-611, LOO-612 |
| A Process row per Desktop poll | **Inefficient today** | LOO-615 |
| Remote Desktop calls wrapped in two `task location` reads; Graph inspector N+1 | **Inefficient today** | file (§4) |
| `wt list` slow enough to need `wt timing` | **Inefficient today**; `wt timing` deleted once fixed | file |
| Captured-history retention | absorbed by LOO-602/614; LOO-459 closes as superseded | LOO-459 |

## 6. Servers and remote calls

Long-running today: `lf ship` (one process per shipping Task), `discord serve`
(foreground poll), `monitor … --watch` and `task watch-files` (NDJSON streams
Desktop holds open), `session serve` (LOO-601; one per Session). `wave cron`
and `ci watch` are gone; scheduled work is a crontab line (`lf pr reconcile`).

**Proposal: `lf serve`, opt-in, stateless.**

- One process a person may run (or install as a launchd/systemd unit). It
  hosts (a) a local socket speaking the CLI — request is an lf argv, response
  is that command's JSON or NDJSON — and (b) webhook endpoints (GitHub checks
  replace `lf ship`/`pr reconcile` polling; Discord replaces `discord serve`).
- **Not lfd again.** lfd owned identity and state and was mandatory. `lf serve`
  owns nothing: the SQLite owner is still the lf library each call runs; every
  operation works identically without the server via plain `lf`; Sessions live
  in their own `session serve` owners, so stopping the server stops no work.
  If a feature only works with the server running, that's the lfd failure
  returning — the rule is the server may only make things faster or push-based.
- **Remote is the same argv.** `--machine` runs the argv over SSH today. A
  server on another machine is the same request over a socket; no third shape.
- A webhook needs a public address; that is the user's tunnel/host choice
  (Tailscale Funnel, cloudflared), not Loopflow-hosted (Jack, 2026-10-11).

Decision D6: build `lf serve` now, or record the rule and ship 1.0 with the
pollers marked experimental.

## 8. Builtin skills and Graphs

Part of the surface because `lf <name>` runs them.

- Root-help "Run" line shows `lf pursue`. Main (#1652) deleted the `ship` and
  `deploy` Graphs; `lf ship` is now a root command (kept beside `task`/`wave`,
  questions).
- Wave VSM skills `s1`–`s5` and Graph `vsm-operate`: names a recruit can't
  guess; experimental or delete.
- Session skills (`task_session`, `wave_session`, `repo_session`) and operators
  are launched by `lf resume`/`lf open`, never by name — hide from `lf list`.
- Graph-step skills (`loop-or-next`, `xor-route`, `token-compress`,
  `loopflow-validate`) — hide from `lf list` (they're Graph internals).
- Workflows `code`, `feature`, `research`: keep.

(Full skill-by-skill pass after D1–D3.)

## Decisions for Jack

Status (2026-10-26): D1 decided (no experimental tier: promise or delete),
D3 superseded by LOO-604, D4 decided (crontab), D5 decided (`repo` dissolved),
D6 decided (build `lf serve`), D7 decided (chunked follow-ups, each lands real
code). D2's new name stays with LOO-618.

- **D1.** What 1.0 promises: the 16-entry root above as stable; no
  experimental tier (Jack: promised or deleted).
- **D2.** `lf resume` as the one resume verb; `task resume` (clears `defer`)
  renamed? (decided below; the new name is open, see §2.)
- **D3.** GUI API as a named surface (§4) — what Desktop calls becomes the
  documented build-your-own-UI API.
- **D4.** Where scheduled jobs live: `lf schedule …` (new noun), `lf machine
  cron …`, or folded into `lf serve`.
- **D5.** `repo` disappears (planning → `wave plan`, CI → `pr ci`, release →
  experimental, tokens → skill)?
- **D6.** `lf serve` now, or the rule only.
- **D7.** Split implementation into stacked Tasks: (a) deletions/merges with no
  behavior change, (b) resume, (c) GUI API + Desktop migration, (d) server.

## LOO-610 implementation scope (operator, 2026-10-26; Jack said "start building")

This Task lands the decided changes only. Everything else in this document stays
design for the follow-ups. No compatibility aliases; Desktop, skills, docs,
`docs/lf-reference.md` (regenerate) and tests move in the same diff.

1. **Delete `wave cron`** (`disposition add list publish unpublish sync history
   trigger remove`) and the hidden `cron run --scheduled` launchd entry, with
   their store tables/receipts code if nothing else reads them (schema change
   via `scripts/new_migration.py`). Desktop's `cron sync --repo`
   (`RegistryQuery.swift:300`) and any UI for it go. Docs that teach crons
   (`docs/waves.md`, `docs/lf.md`, `docs/getting-started.md`,
   `docs/troubleshooting.md`, `builtins/ops/skill/init.md`, `.lf/directions/
   release-evidence.md`) show a plain crontab/launchd line running an ordinary
   lf command instead. `repo ci watch --install` is not a cron; it moves (item 2).
2. **Dissolve `lf repo`** (Jack: repo is the scope of a Task without a Wave, a
   selector like `--wave`, not a command group):
   - `repo release *` → `lf ops release *` (Jack Heart: software ops).
   - `repo ci` → `lf ops ci`. `repo ci watch` and Desktop's `CIWatchers`
     went with main's deletion of `ci watch` (#1652).
   - `repo tokens` → `lf ops tokens` (Jack Heart, 2026-10-26: not `history`, which
     is Session history per LOO-604). Desktop: `RegistryQuery.swift:605,613`.
   - `repo comment` → `lf session steer` with no id (see "One steer verb").
   - `repo refresh` → delete; `wave status --sync` already refreshes
     (Desktop `RegistryQuery.swift:579` uses it).
   - `repo planning key|status` → `lf config planning key|status` (doctor's JSON
     has no planning destinations; see questions).
   - `repo identity` → `lf config repository [--path] [--json] [--bind]
     [--relocate]`. Deleting `--bind`/`--relocate` waits for Git-locator
     identity (rentals bind their clone); planning-surface follow-up.
   - `repo connect`, `repo reteam` → `lf wave connect`, `lf wave reteam`.
   - `repo new-chapter` → `lf wave new-chapter --all` (Desktop
     `RegistryQuery.swift:218`). Cutting chapters is the planning-surface Task.
3. **Introduce the `ops` group** (Jack Heart, 2026-10-26): `lf ops pr`, `lf ops
   release`, `lf ops ci`, `lf ops commit`, `lf ops sync`, `lf ops wt`, `lf ops
   machine` (with `pool`, `ondemand` folded in as `lf ops machine pool|ondemand`),
   `lf ops account`. Canonical paths change; elision keeps daily typing:
   `lf land`, `lf pr land`, `lf commit`, `lf sync`, `lf wt create`, `lf account`,
   `lf machine list`, `lf release run` must all still resolve (verify middle-owner
   elision works; extend the resolver if `lf pr land` does not). Skills, docs and
   agent guidance keep the shortest unique form. Global `--machine`/`--account`
   flags are unchanged.
4. Verify: `lf --help` root no longer lists `repo`, `pr`, `wt`, `sync`,
   `commit`, `machine`, `pool`, `ondemand`, `account`, `cron`; it lists `ops`; reference regenerates;
   `documented_commands` passes; Swift builds; affected Rust/Swift tests pass.

Demo: `lf --help` (no `repo`; `ops` holds delivery and environment), `lf land` /
`lf release status` / `lf ops ci` / `lf ops tokens` work; Desktop's Telemetry,
planning and CI views still load.

## Docs and README redo (Jack Heart, 2026-10-26: part of this Task)

README and `docs/` follow the reorganized surface, in the same PR.

- Structure by what you work with: **Work** (`task`, `wave`), **Sessions**
  (`session`, resume), **Orchestration** (`graph`, `skill`, `lf <name>`),
  plus **Telemetry** (`history`, usage) and **Surfaces** (`open`, `desktop`,
  `--watch`) and **ops** (`pr release ci tokens commit sync wt machine
  account`). Context and Skill Graphs remain README benefits, not sections.
- Benefit first, examples carry it, shortest unique commands (`lf land`,
  `lf commit`, `lf account`), and claim only what exists today: no `lf resume`
  takeover/`session new` (LOO-618), no `--watch` (LOO-604), no front door
  until they land.
- Covers LOO-509 and LOO-404 in `docs/`; they close into this one or keep only
  what's left. Website stays LOO-513 (its homepage still lists three builtins).
- Keep Jack's own wording unless its surface changed; flag, never silently rewrite.

**README voice (Jack Heart, 2026-10-26).** Opens with "A software instrument"
and "arrange and conduct". At its core lf is an agent launcher and session
manager; Skills and Graphs are introduced early, as what you launch. Every
example must visibly do something only lf does (`lf resume` alone shows
nothing; `lf resume --all`, `--task`, a provider ID do).

### Docs map (in place)

**Staged, not all at once (Jack Heart, 2026-10-26).** Teach in stages, one idea
per page. Superseded for the README opening by **Task first** (Jack Heart,
2026-10-26): a Task is one conversation with a workflow behind it, plus its
PR/branch/issue and history; Waves set Task defaults; `lf :` comes last.

Implemented 2026-10-26; the page list is `docs/index.md` "Read by area".

## Task plan (draft; each lands real code)

| Task | Lands | Depends on |
|---|---|---|
| **LOO-610 (this)** | The design, plus: `wave cron` deleted; `lf repo` dissolved (scope above); the `lf ops` group (`pr release ci tokens commit sync wt machine account`, `pool`/`ondemand` under `machine`); reference, docs, skills, tests regenerated. Not in this slice: `clone`, `portfolio`, `scratch`, `monitor` | — |
| Resume (LOO-618) | `lf resume` per §3; renames `task resume` (clears `defer`); deletes `session attach`, `--try/--takeover/--if-free`; absorbs LOO-424 | D2 |
| Usage | `lf usage` filter × group × bucket × measure; interactive-Session usage gap fixed or explained | LOO-611, LOO-612 for `source` |
| GUI API → **LOO-604** | `--watch` on ordinary reads; Desktop moves onto them (Jack Heart: LOO-604's approach). Left here: `lf open --files/--graphs` | — |
| `lf serve` | Opt-in stateless server: CLI-over-socket, GitHub webhook replacing `pr ci` polling; Desktop uses it when present | GUI API |
| Planning surface | `wave` planning plumbing (`edit-plan update-plan new-chapter ensure bind-project connect reteam`) grouped or cut; `config repository --bind/--relocate` deleted once identity resolves by Git locator | — |
| Code host port (filed, local `985879`, Linear sync uncertain) | One code-host + CI port; a second host proves pr/land/ci/release end to end | LOO-610 `pr`/ops surface |

## Concept review: the front door (2026-10-26, open)

Walk: Jack comes back Monday. Experience wanted (1.0 promises: Task management
that just works, Session management): see what needs him, what finished, what
is running, and get into one of them.

Today (verified 2026-10-26 on this machine):
- `lf` alone starts a new `default` conversation (`bin/lf.rs:342`).
- `lf monitor` prints ~200 lines: 125 finished, 58 stopped, 1 running; the
  few that need a person are buried.
- `lf session list --available` → "No Sessions"; `lf open --available` opens
  them all; `lf wave status` / `lf task status` per scope.
- `lf resume` (LOO-618) gets you into one, but only once you know which.

Five readers answer "what needs me" and none is the obvious first command.

Proposed (not accepted): one front door, the attention state "Available" as
its spine.

```
lf                # what needs you, what's running, what finished since you left (this repo; --all for every repo)
lf resume         # get back into the one you were in
lf resume LOO-610 # or the one you picked
lf : "…"          # start something new (today's bare `lf`)
```

Consequences: `monitor` (bare, `active`, `ps`, `top`) folds into bare `lf`
plus `lf ps` for the process tree; `session list --available` and
`open --available` become filters of the same read; Desktop's sidebar and the
terminal front door are one query (`lf watch work`). "Since you left" needs a
last-seen mark per person per scope (small fact, no new store shape).

Open product choice (Jack): does bare `lf` stay "start a conversation", the
way bare `claude`/`codex` do, or become "show my work"? Counterexample to the
proposal: muscle memory from the harnesses, and agents trained on `lf` meaning
"talk to me".

## `ops` stays (Jack Heart, 2026-10-26)

Considered splitting `ops` into `code` (commit, sync, wt, pr, ci, release) and
`capacity` (machine, account). Jack: keep `ops` for now; fewer higher-level
concepts is generally better.

Resuming a Task's conversation stays one verb on the Session: `lf resume
LOO-610` or `lf --task LOO-610 resume`; no `task resume` (proposed, follows D2).

## Alignment with LOO-604 and LOO-594 (2026-10-26)

**Decided (Jack Heart, 2026-10-26): use LOO-604's approach.** No `lf watch` or
`lf view` nouns; live reads are `--watch` on ordinary commands; the GUI API is
the CLI. §4's GUI-API table and the "GUI API" row of the Task plan are
superseded and owned by LOO-604. LOO-594 notes below remain proposals.

LOO-604 (one lf API for Loopflow state) carries Jack Heart's decisions that
supersede parts of §4–§5 here:

- **Live API is `--watch` on ordinary reads** (Jack, option B): `lf task list
  --watch`, `lf session list --watch`, `lf task status <id> --watch`, the
  live-process read; `lf monitor work` deleted. → drop this review's `lf watch`
  noun and `lf view`; the GUI API is the ordinary CLI plus `--watch`. No
  separate GUI surface to document beyond that.
- **`lf history` is Session history** (Jack): Session records tagged with
  Task/Wave/Project; Work changes live on the Task. → `history tokens`
  (codebase measurement, added by this Task's first slice) does not fit; it
  belongs with the codebase (`ops`) or goes. Usage stays under history, so the
  telemetry stories read `lf history usage --by …` unless `lf usage` is
  promoted.
- **Open Tasks are the live set; `lf task list [query] --done --since`** pages
  closed ones (Jack). Fits the Work pillar; `wave show`/`wave status` reduce to
  Wave fields plus `task list --wave`.
- **Conflict:** LOO-604 keeps `lf repo planning status` as the peer-planning
  read; this Task moved it to `lf config planning status`. Whichever lands
  second renames in its Desktop call (`RegistryQuery.peerPlanningStatus`).
  Both branches edit `RegistryQuery.swift` and `docs/lf-reference.md`.

LOO-594 (Desktop Wave view as a standup board, accepted):

- Its board (Done in the last N days · Active · Backlog, against the chapter's
  KRs) is the same question as this review's front door ("what moved, what's
  moving, what needs a person"). Proposed: the terminal front door renders that
  standup read for the current scope (Wave, else repository), from LOO-604's
  `task list` reads; Desktop and terminal share one query.
- It grows `lf desktop` (`enter`, `back`, `send -p session`, `-p board|session|
  tiles`): consistent with host-native pane control. `desktop send -p session`
  types into a pane; `lf session steer` is the portable way to reach a
  conversation.

## Decided additions to LOO-610 (Jack Heart, 2026-10-26)

Built: (4) builtin `code` is the default Workflow when the Task, its Wave and
`.lf/config.yaml` name none; (5) agents hand off changes as Tasks
(`skills/loopflow/`, `builtins/LOOPFLOW.md`); (6) docs and README in place;
(7) builtin `auto` = `graph: pursue` then `graph: deliver`, no stages, one
route to `end` approving Auto-Ship. `auto` is interim until LOO-622 makes auto
mode "any Graph, run past its interactive stages"; then this builtin goes.

## Story, auto mode and configuration (Jack Heart, 2026-10-26)

- **Story:** Loopflow launches agents for you (and your agents launch more),
  organizes them around Tasks, which unify PRs, task tracking, worktrees and
  remote branches, and Waves break the work into sections with their own
  assignments: Graph, account, machine.
- **Sell auto mode.** The main thing to sell is that work runs without you.
- **Configuration is Wave configuration.** Add Wave-wide configurability for
  everything (Graph, agent, machine, accounts); repository-wide follows because a
  repository is the Wave of Tasks without one. Don't build a separate
  repository config path. (LOO-625.)
- **Workflow = your chosen Graph.** A Workflow is fundamentally a Graph; its
  interactive stages happen in the app or the Task conversation, not inside the
  process. Human/interactive flags must be consistent everywhere, and a Graph
  annotated with interactive stages works both when you take part and when you
  just run it. (LOO-622, plus the existing Task Jack recalls on human vs
  interactive flags.)

## One steer verb (2026-10-26, built)

`lf session steer SESSION "…"` steers one Session; without an id it steers the
Wave named by global `--wave`, else the repository from global `--repo` or the
cwd. `wave comment` and `session steer`'s boolean `--repo` are deleted (the
boolean spelled the global scope selector twice; Jack: `--repo` is a scope
selector like `--wave`). `task comment` stays: it publishes the Task's thread,
and its `--steer` keeps meaning "also direct". Desktop calls neither deleted
form.

Unresolved, outside this Task's slice (do not block publish): bare `lf` as
front door vs. "start a conversation" (above); promise-or-delete (no experimental tier; Jack: "promised or deleted") for (`discord`, `machine pool/ondemand`, `wave plan`, `desktop` pane
control, `history replay`) and the not-yet-reviewed roots `clone`,
`portfolio`, `disk`, `scratch`, `monitor`.

## Concept review of the built slice (2026-10-26, built)

Root help is ordered by pillar (enum order in `lf/mod.rs` is help order): Work
`task wave ship` · Sessions `session open :` · Orchestration `graph skill` ·
Telemetry `history` · Surfaces `desktop monitor` · `ops` · `config self list
help context` · not-yet-reviewed `clone portfolio disk scratch discord`. No
names or paths changed. Left: `lf ship` → `lf task ship` (main #1652 chose the
root; see questions).

## Concept review after #1704 and items 4–7 (2026-10-26, built)

Items 4–7 and the Task-first README match the decisions. Main's #1704 brought
`machine use`'s boolean `--repo`, now deleted as `session steer`'s was:
`lf --repo <name> machine use mini [--tasks|--session]` (`Cli::selects_repository`).
Scope order: `--task`, `<wave>`, a selected repository, then the worktree's
Task/Wave/repository. The positional `<wave>` stays (matches `wave workflow
set <graph> <wave>`). The copy to your other Machines forwards `--repository ID`.

Left to their owners: `session connect --task/--wave` (README `lf resume --task`)
repeats the global `--task`/`--wave` — LOO-618 reshapes resume; per-command
`--repo NAME` on `wave rename`, `ops account route [set]` — LOO-625 (Wave
configuration). `ops ci --repo` (a GitHub owner/repo) and `self sync-skills
--repo` (export target) mean something else and stay.

## Run notes

- `wave cron` deletion merged by hand from a side worktree; orphaned cron
  code, tests and fixtures removed.
- Verify run: integration 436/436 after 7 fixes; two_people, golden_prompt,
  clippy, Xcode build-for-testing, SwiftPM build pass. Lib suite stopped at
  966/2215 under machine load (its 9 failures passed in isolation).
- Item 6: docs 5/5, website 72/72, every documented `lf` path resolved.
- Merge with main `e636ab6397` (#1652) and docs port (`964b288617`); steer
  verb (`session_steers`, `global_commands` 45/45, clippy clean).
  Gate owes integration, lib and Swift since the #1652, #1704 and #1707 merges.
- #1704 merged as `b18510b1cb`, #1706/#1707 as `b5ea024fb7`, rentals under `lf ops machine`.
- Compress after `auto`: dropped redundant auto checks (definition list,
  catalog lookup); `local_planning::{auto_workflow,wave_workflow_set,unconfigured}` 3/3.
- Machine-use `--repo` cut + compress: `machine_commands` 29/29, clippy clean.
- Realign after `b5ea024fb7`: `reference_matches_the_compiled_cli` and `documented_commands` 4/4 pass.

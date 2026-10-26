# LOO-610 — The lf CLI, top down

Status: **decided slice in progress** (2026-10-26): `lf ops` group and `lf repo`
dissolution implemented on this branch; `wave cron` deletion in a parallel
worktree, to merge. Deviations recorded in `scratch/questions.md`. Source: `docs/lf-reference.md` at main `1b137b8f2d` (26 top-level
commands, ~230 subcommands, 24 root flags), `lf --help`, the builtin catalog,
LOO-601's branch (hidden `__` commands) and LOO-602's accepted tracking decisions.

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
| `repo` | **split** | Grab bag of 10. Planning bits → `wave plan`; `ci` → `pr ci`; `tokens` → skill; `release` → experimental (§7). `repo` disappears from root. |
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
| `pause` / `resume` | **rename** → `hold` / `release` | Frees "resume" for Sessions; "hold" is already the status word. |
| `claim` / `unclaim` | keep (claims are real ownership) | — |
| `automate` / `automation` / `reconcile` | **merge** → `task status` shows automation; `lf pr ci` owns repair on/off | Three commands, one fact (does CI repair run for this Task). |
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
needed). `request-review` → `reviews --request <login>`. `reconcile` (settle
recorded landings once) is what `ci watch` and `task reconcile` already run —
delete both commands, keep the function. `repo ci` + `repo ci watch` → **`lf pr ci`** (`--watch`, `--status`).

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
- `--fresh` → `lf session new [--task|--wave]`; `task pause/resume` →
  `task hold/release`; `lf connect` stops being a separate name.
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
- `repo ci watch --parent-pid` spawns one watcher per open repository. → `lf
  serve` (§6) or one watcher per Machine.

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
| `history show --events` (verbatim provider stream) | **Wrong promise** — delete | LOO-613 |
| Unbounded transcript history in Loopflow's store | **Wrong promise** — providers retain; lf reads them | LOO-614 |
| `disk rotate` / `disk_budget` covering provider homes lf can't rotate | **Wrong promise** — budget only what lf owns, or drop it (LOO-602 D6) | this Task (API) + LOO-602 D6 |
| `history replay` of any run, forever | **Wrong promise** — replay holds while the capture exists; say so in help | this Task (help text) |
| `session list --limit 0` "complete matching inventory" | promise sound at facts-only scale | — |
| `history usage` / `--context` reading two usage copies + raw stream | **Inefficient today** | LOO-611, LOO-612 |
| A Process row per Desktop poll | **Inefficient today** | LOO-615 |
| Remote Desktop calls wrapped in two `task location` reads; Graph inspector N+1 | **Inefficient today** | file (§4) |
| `wt list` slow enough to need `wt timing` | **Inefficient today**; `wt timing` deleted once fixed | file |
| Captured-history retention | absorbed by LOO-602/614; LOO-459 closes as superseded | LOO-459 |

## 6. Servers and remote calls

Everything long-running today is a separate poller with its own install story:
`repo ci watch --install` (launchd), `wave cron` (launchd), `discord serve`
(foreground poll), `monitor … --watch` and `task watch-files` (NDJSON streams
Desktop holds open), `session serve` (LOO-601; one per Session).

**Proposal: `lf serve`, opt-in, stateless.**

- One process a person may run (or install as a launchd/systemd unit). It
  hosts (a) a local socket speaking the CLI — request is an lf argv, response
  is that command's JSON or NDJSON — and (b) webhook endpoints (GitHub checks
  replace `ci watch` polling; Discord replaces `discord serve`).
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

## 7. Experimental for 1.0

Not deleted, but outside the 1.0 promise, under `lf experimental …` or a hidden
flag — Jack's call (D1): `repo release *`, `discord`, `machine pool/ondemand`,
`wave plan *`, `desktop` pane control, `history replay`.

## 8. Builtin skills and Graphs

Part of the surface because `lf <name>` runs them.

- Root-help "Run" line shows `lf pursue`. Keep `pursue`, `ship`, `deploy`?
  `deploy` (gate → land) and `ship` overlap — merge.
- Wave VSM skills `s1`–`s5` and Graph `vsm-operate`: names a recruit can't
  guess; experimental or delete.
- Session skills (`task_session`, `wave_session`, `repo_session`) and operators
  are launched by `lf resume`/`lf open`, never by name — hide from `lf list`.
- Graph-step skills (`loop-or-next`, `xor-route`, `token-compress`,
  `loopflow-validate`) — hide from `lf list` (they're Graph internals).
- Workflows `code`, `feature`, `research`: keep.

(Full skill-by-skill pass after D1–D3.)

## Decisions for Jack

- **D1.** What 1.0 promises: the 16-entry root above as stable; §7 as
  experimental (namespace `lf experimental …`, or hidden)?
- **D2.** `lf resume` as the one resume verb, with `task pause/resume` →
  `hold/release`?
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
   - `repo ci`, `repo ci watch` → `lf ops ci`, `lf ops ci watch` (Desktop:
     `CIWatchers.swift:31`).
   - `repo tokens` → `lf history tokens` (Desktop: `RegistryQuery.swift:605,613`).
   - `repo comment` → `lf session steer --repo` (steer the repo scope's Sessions).
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
`lf release status` / `lf ci watch --status` / `lf history tokens` work; Desktop's Telemetry,
planning and CI views still load.

## Task plan (draft; each lands real code)

| Task | Lands | Depends on |
|---|---|---|
| **LOO-610 (this)** | The design, plus the decided deletions: `wave cron`, `repo tokens` → `history`, `clone`, `portfolio import`, `scratch` command, `monitor` → `ps`, `pool`/`ondemand` → `machine`; reference, docs, skills, tests regenerated | — |
| Resume (LOO-618) | `lf resume` per §3; `task hold/release`; deletes `session attach`, `--try/--takeover/--if-free`; absorbs LOO-424 | D2 |
| Usage | `lf usage` filter × group × bucket × measure; interactive-Session usage gap fixed or explained | LOO-611, LOO-612 for `source` |
| GUI API | `lf watch`, `lf view`, `lf task files …`; Desktop moves onto them; `lf open --files/--graphs`; the build-your-own-UI page | D3 |
| `lf serve` | Opt-in stateless server: CLI-over-socket, GitHub webhook replacing `pr ci` polling; Desktop uses it when present | GUI API |
| Planning surface | `repo` dissolved or slimmed per D5 | D5 |
| Code host port (filed, local `985879`, Linear sync uncertain) | One code-host + CI port; a second host proves pr/land/ci/release end to end | LOO-610 `pr`/ops surface |

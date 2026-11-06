# Namespace survey against the November 5 decisions

I checked every file the brief listed: 32 docs, 18 project README files (vendored copies under `.build`/`.venv` skipped), all 75 builtins, PROMPTS.md, MEMORY.md and GOAL.md. Nothing was edited.

Two facts change how some findings read:
- **Unique names already resolve at the root.** `navigation.rs::resolve_child` accepts any unique command name or prefix typed at the root. So `lf ps`, `lf arm`, `lf interrupt` and `lf mon prune` all work today. Decision 8 makes only `ship land sync commit open resume` canonical at the root, so any other root spelling in docs is a deviation.
- **Some aliases already exist.** `rust/loopflow/src/lf/aliases.rs` maps `ops`, `config show`, `task edit`, `wave add`, `machine id`, `history usage|retention`, `self doctor`, `config planning` and `config repository --path` to their new paths. None of those old spellings appear in docs, except `wave add` in one dated MEMORY entry.

---

## A. Patterns worth writing down as rules

| # | Pattern | Example locations | Proposed rule |
|---|---|---|---|
| A1 | "show", "status" and "get" are mixed. Docs read one Task with `lf task status` (37 uses); `lf task show` appears only in the server tables. Waves have three reads: `wave list`, `wave status` (43 uses) and `wave show` (28 uses). | docs/work.md:89-90; docs/agent-api.md:214-215; rust/loopflow/src/builtins/wave/skill/s1.md:16 | `get` is the API verb and `show` the CLI verb for one record. `list` is for many. Drop `status` as a verb except `lf server status` and `lf account` capacity. |
| A2 | Three flags mean "preview without applying": `--dry-run`, `--preview` and `--plan`. `--plan` clashes with the Planning noun. | docs/troubleshooting.md:227; docs/work.md:236; docs/ops.md:225 | Use `--dry-run` everywhere. Reserve "plan" for Planning. |
| A3 | Commands are previewed, then applied. Skills always run `--dry-run --json` first and then `--json`. | rust/loopflow/src/builtins/ops/skill/start-chapter.md:42/52; init.md:225-226; split-wave.md:74-76 | Every write that can be previewed has `--dry-run`. Skills preview and then apply with identical arguments. |
| A4 | Two ways to name a Task: as the command's own argument (`lf task run LOO-12`) or with the global `--task` flag (`lf --task LOO-12 land`, `lf open --task LOO-12`). | docs/ops.md:17; docs/surfaces.md:182; docs/work.md:36 | `task.*` commands take the issue as an argument. Commands from other nouns reach a Task through the global `--task <issue>` (decision 3). |
| A5 | Task placeholders vary: `<issue>` (65), `<ID>` (34), `ISSUE` (16), `<id>` (16), `<task>` (9), `<ISSUE-ID>`, `<issue-id>`. Example IDs mix `LOO-12` and `INF-123`. | rust/loopflow/src/builtins/ops/skill/init.md:246; wave_operate.md:152; docs/troubleshooting.md:30 | `<issue>` for a Task, `<session>` for a Session, `<wave>` for a Wave. Use one example prefix. |
| A6 | Selector spellings vary. `--repo` and `--repository` both exist, and so does `--for-task`. Selectors sometimes come before the verb (`lf --wave x set`) and sometimes after (`lf update-plan --wave x`). | rust/loopflow/src/work/README.md:18; docs/dev/machines.md:174; docs/troubleshooting.md:18 | One spelling per noun: `--task --wave --repo --machine --session`. Global selectors go before the verb. No `--for-<noun>`. |
| A7 | "open" means three things: show Work in a host (`lf open`), a Task that isn't closed (`state:open`, "open Tasks"), and draft-PR-plus-browser (`lf pr open`). | docs/surfaces.md:9; docs/work.md:96; docs/agent-api.md:193 | `open` the verb only presents something. A Task that isn't closed is "not done" or `state:open`. Write that down in the glossary. |
| A8 | JSON output is described four different ways. | docs/lf-reference.md:27; docs/surfaces.md:125; server/README.md:88-89 | A read's `--json` is one document. `--watch --json` is one line now, then one per change until stdin closes. The API result equals the CLI's `--json` output. |
| A9 | "returns once it is running; `lf task status` follows it" recurs after every launch. | rust/loopflow/src/builtins/task/skill/launch-plan.md:61; task_operate.md:74; wave_operate.md:154 | One sentence: "Launch returns when running; `lf task show <issue>` follows it." |
| A10 | "Read wave/PM context only when the seed names the exact wave, task, project…" is copied into 10 skills. | builtins/task/skill/gate.md:15; project/skill/polish.md:14; task/skill/5whys.md:17 | Move it into LOOPFLOW.md once, using decided nouns. |
| A11 | The Task-description and comment rules, including the "Moved into the September chapter" example, are copied four times. | task/skill/launch-plan.md:79-97; ops/skill/start-chapter.md:105-123; PROMPTS.md:519-553 | Keep one copy in PROMPTS.md, using report/steer vocabulary. |
| A12 | After an uncertain write, docs say to read state again and never resubmit. | task/skill/capture-tasks.md:72; server/README.md:241-244; docs/agent-api.md:175 | Make it one rule: an `outcome_unknown` write is followed by a fresh read, never a retry. |
| A13 | User docs say "conversation" (130 uses in docs/*.md) where the noun is Session (81 uses). | docs/surfaces.md:9-11; docs/sessions.md:9; docs/work.md:36 | Glossary rule: "conversation" is plain English for a Session, and never appears in a command or API name. |

---

## B. Deviations to fix

The server operation tables (`rust/loopflow/src/server/README.md:117-168` and `docs/dev/server.md:~101-152`) deliberately list today's CLI spellings marked "(pending)". They are consistent, so I left them out below.

### docs/ (user docs)

| File | Line(s) | Current | Decided replacement |
|---|---|---|---|
| docs/work.md | 72, 73 | `lf comment LOO-12 "…"` / read the thread | `lf task steer LOO-12 "…"`; read the thread with `lf task show` |
| docs/work.md | 74 | `lf interrupt` | `lf task interrupt` |
| docs/work.md | 78-80, 181, 186 | "A comment … reaches every Session"; tracker "comments" → direction; "create, edit, comment" | steer (comments from the Linear side can stay "comments"; on the Loopflow side say steer/report) |
| docs/work.md | 90, 106, 116 | `lf task status` | `lf task show` |
| docs/work.md | 91 | `lf task diff --files` | `lf task changes` |
| docs/work.md | 107 | `lf history`, `lf ps` | `lf session history`, `lf process list` |
| docs/work.md | 235, 236, 241 | "chapters" | remove (decision 11) |
| docs/work.md | 165, 209 | `lf wave connect` | `wave.planning.connect` (CLI pending) |
| docs/work.md | 190, 197 | "its plan is a Linear Project"; "a plan of Projects and KRs" | "its planning is a Linear Project" (provider object, fine); drop "Projects" |
| docs/work.md | 304 | `linear.initiative` key | `planning.linear.initiative` |
| docs/work.md | 322-335 | "Plan a chapter: Projects and KRs", `lf wave new-chapter` | delete the section; replace with Wave planning (`wave.planning`, `wave.sync`) |
| docs/work.md | 325 | `lf wave update-plan --wave … --plan` | no decided name (see C) |
| docs/work.md | 390 | `lf wave status --sync` | `lf wave sync` |
| docs/getting-started.md | 75 | `lf comment <ID>` | `lf task steer <issue>` |
| docs/getting-started.md | 90, 115 | `lf task status` | `lf task show` |
| docs/getting-started.md | 91 | `lf ps` | `lf process list` |
| docs/getting-started.md | 132 | `lf doctor` | `lf machine doctor` (root alias allowed) |
| docs/index.md | 37 | `lf comment` | `lf task steer` |
| docs/index.md | 38 | `lf task status` | `lf task show` |
| docs/index.md | 96 | `lf wave status infrastructure` (settings sources) | see C (`wave status`) |
| docs/index.md | 119 | "its current Project holds the plan" | "the Wave's planning holds…" |
| docs/index.md | 139 | link text "Ops" | dissolve (PR / Repository / Machine / Account) |
| docs/troubleshooting.md | 18 | `lf graph list --processes --for-task` | `lf task runs` |
| docs/troubleshooting.md | 29, 52, 73 | `lf doctor` | `lf machine doctor` |
| docs/troubleshooting.md | 30, 36, 94, 117 | `lf task status` | `lf task show` |
| docs/troubleshooting.md | 32, 245 | `lf history show` | `lf session history …` (see C3) |
| docs/troubleshooting.md | 33 | `lf ps` | `lf process list` |
| docs/troubleshooting.md | 45 | `lf comment <ID>` | `lf task steer` |
| docs/troubleshooting.md | 46, 103 | `lf interrupt` | `lf task interrupt` |
| docs/troubleshooting.md | 95, 165 | `lf usage --task` / `--context --task` | `lf session usage --task`; `lf task context` |
| docs/troubleshooting.md | 114, 118 | `lf desktop task <ID> --json` | `lf task show` |
| docs/troubleshooting.md | 177, 227-228, 237, 241-242 | `lf monitor prune` | no home (C2) |
| docs/ops.md | 1 (title), whole page | "Ops" | split by noun: PR, Checkout, Repository (release/CI), Machine, Account |
| docs/ops.md | 48-49, 337-338 | `lf submit`, `lf arm` | `lf pr submit`, `lf pr arm` |
| docs/ops.md | 75, 85, 343 | `lf ci` | `lf repository ci` |
| docs/ops.md | 82 | `lf task status`, `lf ps` | `lf task show`, `lf process list` |
| docs/ops.md | 176-215, 344 | `lf release …` | `lf repository release …` (the root shortcut `release` is reserved in navigation.rs:53 but not in decision 8, so decide) |
| docs/ops.md | 225, 253 | `lf sync --plan` | `--dry-run` (A2) |
| docs/ops.md | 352 | link anchor `lf-reference.md#lf-ops` | drop (`ops` dissolved) |
| docs/orchestration.md | 221 | "is on [Ops]" | per-noun page |
| docs/sessions.md | 113 | `lf steer <session>` | `lf session steer <session>` |
| docs/sessions.md | 114-115 | `lf steer "…"` / `lf --wave billing steer` (repository/Wave steer) | no decided operation (C5) |
| docs/sessions.md | 79, 120 | "comments and steers"; "For a Task, `lf comment`" | `lf task steer` |
| docs/sessions.md | 80 | `lf task status` | `lf task show` |
| docs/sessions.md | 173, 195 | "[Ops]" | per-noun page |
| docs/surfaces.md | 131 | `lf task status … comments` | `lf task show`; "thread" |
| docs/surfaces.md | 133 | `lf history --watch` | `lf session history` |
| docs/surfaces.md | 134 | `lf ps --watch` | `lf process list --watch` |
| docs/surfaces.md | 136, 180, 186 | `lf desktop plans`, `desktop view --plan` | `desktop.view.list` / `desktop.view.get` |
| docs/surfaces.md | 147 | Task row field `"project": "Search"` | remove (Project is just Wave) |
| docs/surfaces.md | 168-169 | `lf desktop plan/task/wave/done/sessions` | `lf planning list`, `lf task show`, `lf wave planning`, `lf task list --closed`, `lf session list` |
| docs/agent-api.md | 8 | "Task, Wave and Project APIs" | "Task and Wave APIs" |
| docs/agent-api.md | 24-25, 206, 216, 221 | `lf history`, `lf history show/list` | `lf session history` (C3) |
| docs/agent-api.md | 61, 91 | `lf checkout INF-123` | `lf task checkout` (C6) |
| docs/agent-api.md | 64, 113 | `lf comment INF-123` | `lf task report` (progress, line 64); `lf task steer` (line 113) |
| docs/agent-api.md | 114, 136-141 | `lf comment … --steer`; "`task comment` from an agent Session marks … progress" | `lf task steer`; `lf task report` for progress; `comment` goes away (decision 6) |
| docs/agent-api.md | 65, 153 | `lf task status` | `lf task show` |
| docs/agent-api.md | 72, 148, 159, 164 | "current Project owns Tasks…", "Project operations" | the Wave's planning; "Wave operations" |
| docs/agent-api.md | 144 | `lf interrupt` | `lf task interrupt` |
| docs/agent-api.md | 158 | "re-check current PM" | Planning |
| docs/agent-api.md | 175 | "chapter replacement" | remove |
| docs/agent-api.md | 189 | `lf submit` | `lf pr submit` |
| docs/agent-api.md | 205 | `lf usage --project parser` | `lf session usage --wave` |
| docs/agent-api.md | 207-210 | `lf usage` | `lf session usage` |
| docs/agent-api.md | 211 | `lf ps` | `lf process list` |
| docs/telemetry.md | 15-27, 113-115 | `lf usage` (and `--project` at 113) | `lf session usage` (drop `--project`) |
| docs/telemetry.md | 41-51, 85, 116-119 | `lf history` (and `--project` at 116) | `lf session history` (drop `--project`) |
| docs/telemetry.md | 53 | "on the Task: `lf task status`" | `lf task show` |
| docs/telemetry.md | 79-80, 121 | `lf top`, `lf ps` | `lf process list` (top: C2) |
| docs/telemetry.md | 81, 85, 122 | `lf monitor prune` | C2 |
| docs/telemetry.md | 92-93, 123 | `lf tokens` | `lf repository tokens` |
| docs/telemetry.md | 103-104, 124 | `lf doctor` | `lf machine doctor` |
| docs/glossary.md | 18 | **Chapter** entry | delete |
| docs/glossary.md | 19 | **Project** entry | delete, or "Project: see Wave" |
| docs/glossary.md | 58-59 | "Tasks and Projects are shared" | "Tasks and Wave planning" |
| docs/lf-reference.md | 6 | `lf monitor --json` | C2 |
| docs/lf-reference.md | 27 | `monitor active --watch --json` | stale: there is no `monitor active` subcommand (MonitorCommand has Ps/Top/Rollover/Prune); use a real watched read |
| docs/server.md | 59 | `lf doctor` | `lf machine doctor` |
| docs/security.md | 78 | `lf submit` | `lf pr submit` |
| docs/security.md | 158 | `lf --machine … history list` | C3 |
| docs/subscriptions.md | 143 | `auth status` | `lf account` (account.*) |
| docs/subscriptions.md | 148 | `lf usage` | `lf session usage` |
| docs/subscriptions.md | 315-345 | "daemon" | not a deviation: this is Codex's own daemon |
| docs/config.md | 7-8, 17, 41, 52, 61-63, 95-110, 156, 172-196, 666, 681, 1039-1040 | `skills.<name>.…`, `graphs.<name>.…` keys | canonical `skill.<name>.…` / `graph.<name>.…` (decision 5; code in `config/settings.rs:133-137` is still plural) |
| docs/config.md | 214-220 | `lf set account.claude` "in this project"; "The project list" | account selection is user-scope `wave.<name>.account`/repository key (decision 10). Also: "project" here means repository |
| docs/config.md | 28, 47, 58, 66-68, 262-263 | "project" meaning repository | Repository |
| docs/config.md | 385-386 | `lf user` | C |
| docs/config.md | 444-446 | `lf context` | C4 |

### docs/dev/

| File | Line(s) | Current | Replacement |
|---|---|---|---|
| planning.md | 9, 332-334 | `lf wave new-chapter` | deleted |
| planning.md | 14, 148, 378, 477, 486, 697 | Chapter / rotation | remove |
| planning.md | ~67 "Project" hits, e.g. 14, 286, 309-312 | Project as the Wave's plan | `wave.planning` (keep "Linear Project" only for the provider object) |
| planning.md | 25, 675 | `lf task comment`, `lf comment` | `lf task steer` |
| planning.md | 5, 387, 619 | `lf task status` | `lf task show` |
| planning.md | 276 | `lf wave bind-project` | `wave.planning.bind` |
| planning.md | 388, 391 | `lf desktop task` | `lf task show` |
| planning.md | 51, 286, 291, 309, 397, 639 | "roadmap" | Planning (`planning.list`) |
| planning.md | 89-91 | "no generic PM facade"; `ops/planning_sync.rs`, `ops/task.rs` | PM → Planning; source paths can stay |
| planning.md | 429 | "former daemon webhook" | historical, fine |
| reference-models.md | 135, 472, 487-493 | Chapter | remove |
| reference-models.md | 73, 122, 134, 137, 158, 185-187, 439, 464, 489-491 | Project (Wave → Project → Task) | Wave → Task; provider Project only as binding |
| reference-models.md | 435 | `lf task status` | `lf task show` |
| reference-models.md | 16, 160, 480 | roadmap | Planning |
| reference-models.md | 461 | `lf usage --binds` | `lf session usage --binds` |
| reference.md | 70 | "project selection" | planning |
| reference.md | 71 | "Chapter / Project" row | remove; the Wave row covers it |
| reference.md | 73 | "optional Project membership" | Wave membership |
| reference.md | 181, 198-200 | Chapter | remove |
| reference.md | 162, 171 | `pm.linear_project`, PM | historical retired-name table, fine |
| index.md | 29 | "no Wave, Project, Task, or daemon" | "no Wave, Task, or server process" |
| index.md | 111 | `ops/chapter.rs deterministic chapter rotation` | file deleted with decision 11 |
| index.md | 158, 193, 211-212, 248-249, 258-261, 311, 335 | Project / Chapter | Wave planning; remove Chapter |
| index.md | 283 | `lf checkout` | `lf task checkout` (C6) |
| index.md | 288 | `lf submit` | `lf pr submit` |
| index.md | 289 | `lf task status` | `lf task show` |
| index.md | 174 | "status / roadmap / usage" | planning |
| data.md | 7 | `lf ps --json` | `lf process list` |
| data.md | 134, 156-161 | Chapter, Project | remove / Wave planning |
| delivery.md | 40, 53, 61, 362 | Project membership | Wave membership (keep "Linear Project" at 362) |
| delivery.md | 12, 53, 67 | `lf checkout` | `lf task checkout` |
| delivery.md | 90, 110-116, 274 | `lf arm`, `submit` | `lf pr arm`, `lf pr submit` |
| delivery.md | 248 | `lf task status`, `lf ps`, `lf wave status` | `lf task show`, `lf process list`, C1 |
| delivery.md | 302 | `lf history show` | C3 |
| delivery.md | 47 | "live under `ops/`" | source path (`ops` the CLI noun is gone) |
| codebase.md | 25, 51, 108, 137 | "Task/Project operations", "Project operation" (`ops/project.rs`) | Wave operation |
| codebase.md | 79-80 | `lf history`, `usage`, `ps`, `top`, `prune`, `doctor` | `session history`, `session usage`, `process list`, C2, `machine doctor` |
| codebase.md | 90 | `lf checkout` | C6 |
| reference-code.md | 15 | "Task/chapter control … PM operations" | Task control, Planning |
| reference-code.md | 122 | `lf monitor`, `ps`, `top`, `prune`, `lf history linear` | process list, C2, C3 |
| reference-cutover.md | 59 | `lf history show`, `lf history replay` | C3 |
| reference-cutover.md | 77 | "Status-owned Chapters" row | historical cutover ledger; mark retired |
| reference-runtime.md | 100 | Chapter rotation | remove |
| reference-runtime.md | 108 | `lf comment … --steer` | `lf task steer` |
| reference-runtime.md | 180, 237 | "Project's Graph"; `lf checkout` "inside one Project" | Wave's Graph (`wave.graph`); C6 |
| reference-runtime.md | 245 | `lf submit`, `lf arm` | `lf pr …` |
| reference-processes.md | 86 | `lf mon prune` | C2 |
| reference-processes.md | 188, 192 | "PM Teams" | planning Team |
| reference-processes.md | 193 | `lf reteam` | `wave.planning.reteam` |
| reference-processes.md | 210 | `lf doctor` | `lf machine doctor` |
| machines.md | 69, 73 | `lf history list` | C3 |
| machines.md | 70, 332 | `lf ps` | `lf process list` |
| machines.md | 71, 162 | `lf wave status` | C1 |
| machines.md | 96 | `lf desktop task` | `lf task show` |
| machines.md | 218, 303 | `lf task status` | `lf task show` |
| machines.md | 297, 304 | `lf history export`, `history show` | C3 |
| machines.md | 333-341 | `lf top`, `lf mon prune` | C2 |
| machines.md | 186 | Task/Project IDs | Task/Wave |
| machines.md | 357 | "daemon or bridge" | fine (says none is needed) |
| machines.md | 368 | "PM and Work rows" | planning |
| environment.md | 90 | `LF_LFD_ALLOW_NON_LOOPBACK` "daemon" | `lfd` env var: retire or rename |

### Project READMEs

| File | Line(s) | Current | Replacement |
|---|---|---|---|
| swift/README.md | 36 | `lf doctor` | `lf machine doctor` |
| swift/README.md | 78-79 | "current Project", **Project history**, **Realign Projects…** (UI labels) | Wave planning labels |
| swift/README.md | 390 | `lf usage --days 0 --task` | `lf task context` / `lf session usage` |
| swift/README.md | 394 | `lf task comment ID --json` (thread read) | `lf task show` (thread is in `task.get`) |
| swift/README.md | 468, 484-485, 499, 603, 618, 620 | chapter / internal Project | Wave planning |
| swift/README.md | 502 | "shared PM API" | Planning |
| swift/README.md | 561, 606, 628-629, 642 | roadmap | `planning.list` |
| swift/README.md | 589 | `lf portfolio` | `lf desktop portfolio` |
| swift/README.md | 604 | `lf desktop wave` | `lf wave planning` |
| swift/README.md | 608, 752 | `lf desktop plan` | `lf planning list` |
| swift/README.md | 622-623 | `lf diff --files`, `lf diff`, `lf file` | `lf task changes`, `lf task diff`, `lf task file` |
| swift/README.md | 624 | `lf interrupt` | `lf task interrupt` |
| swift/README.md | 627 | `lf monitor`, `lf history` | `lf process list`, `lf session history` |
| swift/README.md | 753 | `lf desktop task` | `lf task show` |
| swift/README.md | 754, 781 | `lf desktop sessions` | `lf session list` |
| swift/README.md | 781 | `lf desktop plans` | `desktop.view.list` |
| swift/README.md | 782 | `lf history`, `lf monitor ps` | `lf session history`, `lf process list` |
| swift/README.md | 826-843 | "Preparing Project…", `project_readiness`, "Project selectors" | Wave planning readiness |
| rust/loopflow/src/transport/README.md | 85, 104 | `lf monitor rollover` | C2 |
| rust/loopflow/src/transport/README.md | 214 | `lf session attach OWNER` | `process.attach` (CLI `lf process attach`) |
| rust/loopflow/src/work/README.md | 4, 6, 128 | `desktop view … --plan`, "the same plans" | `desktop.view.get`, `--dry-run` |
| rust/loopflow/src/work/README.md | 102 | "the existing Task edit owner" | Task update |
| rust/loopflow/tests/README.md | 277-278, 474 | Chapter convergence, `ops::chapter::tests` | delete with decision 11 |
| rust/loopflow/tests/README.md | 320-321, 506, 611, 668, 823-844 | Project readiness, Project fixtures | Wave planning |
| rust/loopflow/tests/README.md | 194, 208, 691 | PM | Planning |
| rust/loopflow/tests/README.md | 639-640 | `lf history list`, `lf usage`, `lf doctor` | C3, `session usage`, `machine doctor` |
| rust/loopflow/tests/README.md | 644-651 | "Codex daemon" | provider term, fine |

### Builtins (`rust/loopflow/src/builtins/**`)

| File | Line(s) | Current | Replacement |
|---|---|---|---|
| LOOPFLOW.md | 12 | "Task, Wave or Project commands" | Task or Wave |
| LOOPFLOW.md | 18 | `lf task status <ID>`; `lf comment <ID>` | `lf task show <issue>`; `lf task steer <issue>` |
| ops/skill/work.md | 2, 14 | "Task, Wave and Project APIs"; **Project** definition | remove Project |
| ops/skill/work.md | 57 | `lf task status` | `lf task show` |
| ops/skill/work.md | 103-104 | "belongs in Task comments"; `lf comment --steer` | `lf task report`; `lf task steer` |
| ops/skill/start-chapter.md | whole file (2-134), incl. 42, 52, 70-71 `new-chapter` | chapter skill | delete or replace with a Wave-planning skill (decision 11) |
| ops/skill/review-chapter.md | whole file (2-61), incl. 38 `wave-review-chapter`, 54 `new-chapter` | chapter review | rename to a planning review, or delete |
| ops/skill/init.md | 189, 175, 352 | "PM" | Planning |
| ops/skill/init.md | 209, 212-213, 223-238, 344, 362 | chapter, `new-chapter`, internal Project | Wave planning; remove |
| ops/skill/init.md | 173 | `auth status --json` | `lf account …` |
| ops/skill/init.md | 192 | `lf wave connect` | `wave.planning.connect` |
| ops/skill/init.md | 234 | `lf update-plan` | C (undecided) |
| ops/skill/init.md | 247, 330 | `lf task status <ISSUE-ID>` | `lf task show <issue>` |
| ops/skill/init.md | 330 | "machine-wide roadmap" | `planning.list` |
| ops/skill/repo_session.md | 48-53 | Project binding, chapter, start-chapter | Wave planning binding |
| ops/skill/repo_session.md | 70 | `lf sync --plan` | `--dry-run` |
| ops/skill/repo_operate.md | 10 | "chapter rotation" | remove |
| ops/skill/repo_operate.md | 110 | "not Task comments" | not Task reports |
| ops/skill/repo_operate.md | 181 | heading "Diagnose execution and auth" | account |
| ops/skill/repo_operate.md | 183-186 | `lf top`, `lf ps`, `lf mon prune` | process list; C2 |
| ops/skill/pr-land.md | 26, 49 | `lf arm`, `lf submit` | `lf pr arm`, `lf pr submit` |
| ops/skill/release-run.md | 27, 54-64 | `lf release …` | `lf repository release …` (pending the root-shortcut decision) |
| ops/skill/sync-conflicts.md | 13 | wave/PM/project boilerplate | A10 |
| ops/ and project/ directories | — | builtin directories named after the removed `ops` and `project` nouns | rename to repository/ (or pr/checkout) and wave/ |
| task/skill/launch-plan.md | 24, 79-97, 107, 156 | "Wave chapter state/recommendation", "Task comments", "September chapter" | Wave planning; Task reports |
| task/skill/launch-plan.md | 61 | `lf task status` follows it | `lf task show` |
| task/skill/launch-plan.md | 138 | `lf checkout <issue>` | C6 |
| task/skill/kickoff.md | 34, 61, 157 | PM/project boilerplate; "chapter KRs" | Wave planning KRs |
| task/skill/design.md | 24, 72, 99-100 | chapter state; Task comments | Wave planning; reports |
| task/skill/prompt.md | 27, 162, 171-173 | chapter KRs, "internal Project, edited with `lf update-plan`", roadmap | Wave planning |
| task/skill/prompt.md | 103-105 | "comments" for planning updates | reports |
| task/skill/advance.md | 22 | `lf task status` | `lf task show` |
| task/skill/task_operate.md | 28, 74 | `lf task status` | `lf task show` |
| task/skill/task_operate.md | 31 | `lf graph show --processes` | `lf task runs` |
| task/skill/task_operate.md | 46 | `lf ps --json` | `lf process list` |
| task/skill/capture-tasks.md | 58 | `lf checkout` | C6 |
| task/skill/capture-tasks.md | 70 | "its Project unavailable" | its Wave planning |
| task/skill/ship-decomposed.md | 79 | `lf checkout` | C6 |
| task/skill/ci-fix.md | 43, 54, 58, 68 | `lf arm` | `lf pr arm` |
| task/skill/{gate,5whys,implement,research}.md, project/skill/{polish,reduce}.md | 15/17/15/14/14/14 | "wave/PM context … project" boilerplate | A10 |
| task/skill/qa.md | 14 | "repair PM access" | planning |
| wave/skill/wave_start-chapter.md | whole file, incl. 31 `new-chapter` | chapter skill | delete or rename |
| wave/skill/wave_review-chapter.md | whole file | chapter review | rename or delete |
| wave/skill/wave_session.md | 45-50 | Project binding, chapter | Wave planning |
| wave/skill/wave_session.md | 64 | `lf sync --plan` | `--dry-run` |
| wave/skill/wave_operate.md | 10, 24-25 | chapter rotation; "current Project's Tasks … The Project is part of the Wave" | Wave planning |
| wave/skill/wave_operate.md | 29, 81, 154 | `lf task status` | `lf task show` |
| wave/skill/wave_operate.md | 55 | "interrupt or comment on it" | interrupt or steer |
| wave/skill/wave_operate.md | 83 | `lf ps` | `lf process list` |
| wave/skill/wave_operate.md | 183 | roadmap | planning |
| wave/skill/wave-report.md | 14, 149-154, 201 | PM boilerplate, "current chapter", roadmap | Wave planning |
| wave/skill/split-wave.md | 19, 45 | "PM snapshot" | the Wave's planning |
| wave/skill/split-wave.md | 28, 63-68, 82-86 | Linear Projects, Project creation | provider objects; Wave planning |
| wave/skill/split-wave.md | 62 | `lf wave connect` | `wave.planning.connect` |
| wave/skill/split-wave.md | 65, 68 | `lf update-plan` | C |
| wave/skill/split-wave.md | 73-76 | chapter rotation, `new-chapter` | delete |
| wave/skill/s1-s5.md | 16 | `lf wave status` + `lf wave show` pair | C1 |
| wave/skill/s1.md | 24 | `lf task status` | `lf task show` |

### PROMPTS.md

| Line(s) | Current | Replacement |
|---|---|---|
| 298-357 | "Wave/project/task ontology": Project as "measured bet inside one wave", "project KRs", Weak/Strong project examples | rewrite as Wave/Task; KRs belong to the Wave's planning |
| 494 | "Join current Project/KR/Task planning" | Wave planning |
| 519, 529-538, 552 | "Task descriptions and comments"; comments hold progress | Task reports (decision 6) |
| 531, 551-553 | chapter identifiers, "September chapter" example | remove |

### Jack Heart's own text and dated decision records (do not reword)

| File | Line(s) | Term | Note |
|---|---|---|---|
| .lf/wave/product/GOAL.md | 14, 17, 21 (heading), 24, 27, 29, 33 (`lf wave new-chapter`), 35, 54, 57 | chapter, internal Project, `new-chapter` | Jack's Wave goal. Flag it to him: decision 11 makes most of the "Chapter planning" section stale. |
| .lf/wave/product/GOAL.md | 6-7 | `linear: initiative:` frontmatter key | becomes `planning.linear.*`; per decision 5 it may already be readable as a unique shorthand |
| .lf/wave/product/MEMORY.md | 462 | "Jack also accepted ordinary Projects independent of optional chapter coordination (LOO-366)" | dated decision record |
| .lf/wave/product/MEMORY.md | 879 | "LOO-500 (Jack Heart, 2026-10-28): CLI/Desktop share `wave add\|remove\|rename`" | dated decision; supersede with a new entry |
| .lf/wave/product/MEMORY.md | 806 | heading "Chapter decisions and review lessons (2026-09-23)" | dated |
| .lf/wave/product/MEMORY.md | 58, 242, 262, 689, 699, 725, 742, 778, 872-876, 888, 892, 900, 912, 1001 | `edit-plan`, `lf monitor work`, chapter, Project, Comments, `lf pm show`, `lf pm reteam`, lfd, `lf ps` | agent-curated memory of past state. Add a dated supersession entry rather than rewriting. |
| docs/dev/reference-cutover.md | 29-30 | "Jack Heart also confirmed … public Session attachment" | quoted confirmation, no deviation |

No direct Jack quotation in MEMORY.md ("soften, dont harden" at 508, "seems…" at 1097) contains a deviating term.

---

## C. Concepts with no noun in the decided set

| # | Concept | Where | Question for Jack |
|---|---|---|---|
| C1 | **Wave status vs show vs planning.** `lf wave status` is "plan, Tasks, metrics and execution evidence" and "each setting and its source". `lf wave show` "joins the plan to runtime truth". `lf wave show --all` is "every Wave's plan". | docs/work.md:89, 255, 297; docs/agent-api.md:214-215; docs/surfaces.md:129 | Which is `wave.get`, which is `wave.planning`, and is `lf wave show --all` the same as `planning.list`? Does settings-source display belong in `config.settings --wave`? |
| C1b | **`lf task status`** is a separate read today. `task.get` is mapped to `lf desktop task` / `lf task show` (pending). | 37 sites (B) | Is `task status` the same read as `task.get`, or something else? |
| C2 | **Monitor family**: `lf monitor` (waiting/blocked/active overview), `monitor top`, `monitor prune`, `monitor rollover`, `monitor work`. Only `ps` maps (to `process.list`). | docs/telemetry.md:79-85, 122; docs/troubleshooting.md:177-242; transport/README.md:85, 104; docs/lf-reference.md:6 | `process.top/prune`? `session.rollover`? Is the overview `planning.list`? |
| C3 | **Process history vs Session history.** `lf history list` lists commands (Processes). `lf history show ID` takes a Process or a Session. Also `history search/export/replay/linear`. Only `session.history` is decided. | docs/telemetry.md:42-47, 117-119; docs/dev/machines.md:69, 297; docs/dev/reference-code.md:122 | Split into `process.history` / `session.history`? Where do `search`, `export` and `replay` live? |
| C4 | **Context budget**: `lf context` shows memory/scratch size targets. | docs/config.md:444-471; docs/work.md:230 | `config.*`, `task.context`, or `wave.context`? |
| C5 | **Repository- and Wave-scoped steer**: `lf steer "…"` and `lf --wave billing steer`. | docs/sessions.md:114-115; docs/agent-api.md:124-126 | `wave.steer` / `repository.steer`, or `session.steer` with a selector? |
| C6 | **`lf checkout <issue>`** creates a Task's worktree. It collides with the Checkout noun (`checkout.create` = `lf wt create`), and `lf wt` and Checkout are spelled differently. | docs/agent-api.md:61, 91; docs/dev/delivery.md:12, 67; builtins/task/skill/ship-decomposed.md:79 | Is it `task.checkout` (a proxy to `checkout.create`), and is the CLI noun `checkout` or `wt`? |
| C7 | **Server** (`lf server status/run`) and **Installation/self** (`lf install`, `lf self install`, `lf self sync-skills`, `lf schedule`). | docs/server.md; docs/ops.md:104-132, 256-260; docs/config.md:932; docs/glossary.md:29 | Under Machine (the README says Machine owns server client access), or new nouns? |
| C8 | **Wave plan editing**: `lf update-plan` / `lf wave update-plan` / `lf wave edit-plan` (KRs, metric targets). | docs/work.md:325; builtins/ops/skill/init.md:234; split-wave.md:65 | `wave.planning.update`? Do KRs survive decision 11 at all? |
| C9 | **Wave metrics/instruments, KRs, targets** (`metric_portfolio`, "Live metric"). | docs/dev/reference.md:72; docs/agent-api.md:202-203; wave_review-chapter.md:37 | A field of `wave.planning`, or `wave.metrics`? |
| C10 | **Steer delivery read/retry**: `lf session steers <session> [--retry]`. | docs/sessions.md:116, 123; docs/agent-api.md:115-116; docs/dev/reference-runtime.md:112-115 | `session.steers` / `session.steer.retry`? |
| C11 | **Scratch / working notes**: `lf scratch clear`. | docs/config.md:419, 970; docs/ops.md:55; docs/glossary.md:33 | `checkout.scratch`? |
| C12 | **Discord bridge / Wave chat**: `lf discord serve`. | docs/work.md:350-361; docs/security.md:163 | `wave.chat`? |
| C13 | **`lf user`** (whoever is acting) and **Pools/on-demand** (`lf pool`, `lf ondemand`). | docs/config.md:385; docs/ops.md:286-289, 347 | `account.self`? `machine.pool` / `machine.ondemand`? |
| C14 | **Dashboard "plan"**: `WorkViewPlan`, `desktop plans`, "Dashboard plans" use "plan" for a Dashboard's evaluated membership. | swift/README.md:781; work/README.md:128; server/README.md:194-195 | Rename to "view" or "membership" so "plan" means only Planning? |
| C15 | **Root shortcuts outside decision 8**: `release`, `set`, `unset`, `comment` are hard-coded in `navigation.rs:51-56`. `comment` must go per decision 6. | rust/loopflow/src/lf/navigation.rs:51-56 | Keep `lf release` and `lf set/unset` as decided shorthands, or make them aliases? |

# v0.10.0

This is the release where the **Wave** becomes loopflow's core primitive: a persistent, goal-driven looping agent that reads one `goal.md`, keeps its own `MEMORY.md`, works an Asana roadmap, and dispatches steerable sub-sessions instead of solving inline. Around that idea the runtime collapsed hard — the lifecycle model shrank to three nouns (Wave, Run, Session), and Postgres, `lfq`, the Docker `lfd` path, the conversations subsystem, Linear/Notion, and studio auth all left the tree. The result deletes far more than it adds: a self-hosted-by-default product with one authoring surface and a much smaller surface to run. If you upgrade for one reason, upgrade for the Wave.

## The Wave: a goal-driven looping agent

A wave used to be a config row scoped by `area × flow × direction`, woken by a cold ticker. It is now the looping orchestrator itself. You author one file — `wave/<name>/goal.md`, intent in frontmatter, the loop prompt as body — and the wave runs it as a persistent mind: reading the roadmap, capturing the next move as a task, then dispatching a flow against that task as a session you can attach to and steer. It talks back through a chat/memory speech surface rather than a transcript you have to dig through.

- **`lf goal` and `lf wave` launch and drive looping waves** — `lf goal` starts a wave's goal agent via `lf op dispatch`; `lf wave` adds a foreground progress runtime so a loop's turns, state, and memory are visible as it works (#757, #752, #778).
- **A reactive server, one persistent mind** — the goal loop is replaced by a persistent reactive server with a chat/memory speech surface and a harness engine, so a wave is a single continuous agent rather than repeated cold runs (#796).
- **`goal.md` + `MEMORY.md` are the wave's authored and remembered state** — one human-authored intent file per wave; server-owned curated memory the agent maintains and never hand-edits (#752, #782).
- **Chords are just waves with children** — wave ancestry is back, so a parent chord surfaces its children through `parent_wave_id`; no separate chord entity, and the vocabulary is parent/child/sibling (#781).
- **VSM system charters ship as builtin goals** — `s1..s5` govern charters are available as builtin goals for structuring a wave hierarchy without authoring them from scratch (#765).
- **A local run ledger** — `lf runs` and `lf trace` record and inspect each dispatched run locally, so what a wave launched is auditable after the fact (#797).

## A smaller runtime: Wave, Run, Session

The lifecycle model had grown five overlapping nouns (`WaveRun`, `AgentRun`, `TerminalSession`, `Session`, `AgentLaunch`). It is now three. **Wave** is durable goal/memory identity, **Run** is one agent invocation's execution and PR lineage, **Session** is the attachable live conversation surface. `/v0/wave_runs` and `/v0/terminal-sessions` collapse into `/v0/runs` and `/v0/sessions`. Alongside the rename, whole subsystems that no longer earned their place were removed outright — this release is a net deletion of tens of thousands of lines.

- **Three nouns end to end** — Rust core types, HTTP DTOs/routes, Python/`lfq` models, Swift/Concerto models, and fixtures all move to `Run`/`Session`/`run_id`/`session_id` (#759).
- **The `lfd`/`lfq`/`lfdb` collapse** — the daemon, queue, and database layers realign into one coherent runtime instead of three drifting ones (#801).
- **Postgres, `lf q`, and the Docker `lfd` path are gone** — SQLite is the backend; the standalone queue command and the container deploy path retire in favor of the self-hosted native/remote model (#803).
- **The dormant conversations subsystem is removed** (#766), and **RLM** — the ~150-line map-reduce playbook injected into *every* prompt — is deleted; the looping Wave is now the framework for running sub-agents (decision ledger).
- **Asana is the only PM backend** — Linear, Notion, the down-mirror PM tables, and ingestion are removed; the roadmap lives in Asana and the wave reads and writes it directly (#764).

## Self-hosted by default

Remote `lfd` is now yours to run, authenticated by a single explicit token. Studio auth, daemon registration, and hosted discovery are gone; each repo owns its deployment config and keeps secrets in Doppler or host-local env. On top of that base, this cycle built the native Mac host story: a manager, scheduled self-updates, plist hygiene, and spend guardrails.

- **Bearer-token auth only** — `LFD_AUTH_TOKEN` is the lone remote-auth knob; `AuthMode`, the `/v0/tokens` routes, and `lfq token revoke` are removed (#721).
- **TLS-fronted remote lfd from Concerto** — connect to a self-hosted host over TLS/Caddy/Tailscale with an explicit URL + token (#755).
- **A native Mac `lfd` host manager** — bring up, update, and manage a native host; scheduled updates keep it on the default branch, and tokens stay out of launchd plists (#740, #742, #743, #734).
- **Cron host bootstrap + docs** — a bootstrap script and a hardened Mac Mini setup guide, with private host details scrubbed from the committed docs (#724, #727, #738, #729).
- **Monthly spend guardrails** — stdlib-only spend caps to keep a self-hosted host from running away (#733, #735).

## Concerto and the desktop app

The desktop app is now **Loopflow.app**, and it launches real repo-backed waves through `lf tmux` rather than an in-app runtime. Navigation and window scoping got sharper so a window always reflects one repo.

- **Repo-backed waves through `lf tmux`** — Concerto launches waves as attachable tmux sessions, aligning the desktop with the CLI's session model (#763).
- **Renamed to Loopflow, one build entry per worktree** — the app is Loopflow.app, and local builds are scoped per worktree so parallel work doesn't collide (#744).
- **Deep-link and menu navigation** to open a specific repo or the portfolio (#728), with the connected wave snapshot **scoped to the window's repo** so windows don't bleed state (#732).
- **`concerto-dev run-debug --repo`** launches the debug build straight into any repo (#736).

## Release, install, and CI machinery

The automation spine got boring on purpose. Weekly publishing runs the canonical release flow, the local update path is one command, and `lf op` now plans a rebase before touching git. CI shed close to a minute-and-a-half of wall time per run.

- **Canonical release flow for weekly publishing** — the scheduled path uses `lf op release run` (create release PR → wait for merge → tag → wait for publish) instead of a bespoke workflow (#748).
- **Plan-first rebase and worktree placement** — `lf op rebase --plan` and `lf op wt create` classify the branch and print the deterministic reset/rebase/placement decision before mutating git (#802).
- **One shared local refresh path** — `install.py refresh` is the shared update command for laptops and native hosts, it ignores stale pull config, and it syncs agent skills after every install so `~/.claude/skills` tracks the freshly built binary (#750, #741, #779).
- **Repo-grain token usage + `lf usage`** — per-repo token accounting with a CLI surface to read it (#792).
- **Close roadmap tasks with a PR link** — `lf op pm update --pr <url>` marks an Asana task done and comments the shipped PR without clobbering its description (#780).
- **CI is faster and green** — `cargo-nextest` for Rust (~15–30s/run), `-gnone` on the Swift test build (~10–15s), a trimmed Concerto UI test host (~45s) that now exits cleanly instead of relying on an idle-kill hack, and a changed-aware `scripts/test.py` runner for the gate loop (#787, #788, #785, #790, #789, #795).

## Operational notes

This is a minor bump with real breaking changes — no backwards-compat aliases were kept.

- **Set `LFD_AUTH_TOKEN`.** Remote `lfd` requires it; studio auth, daemon registration, hosted discovery, and `LFD_AUTH_MODE` are removed. Point clients at explicit URLs + tokens (#721).
- **HTTP routes moved.** `/v0/wave_runs` → `/v0/runs`, `/v0/terminal-sessions` → `/v0/sessions`; DTOs and models use `run_id`/`session_id`. Update any external caller (#759).
- **Postgres and the Docker `lfd` path are gone** — migrate to SQLite and the native/remote self-hosted deploy path; `lf q` is removed (#803).
- **Linear and Notion are gone** — the roadmap is Asana-only; a wave needs an Asana project to carry a roadmap (#764).
- The launchd plist is `loopflow.server.plist`; native host tokens live in host-local env, not the plist (#721, #743).

## Small changes

- **`engine/git` no longer reverts just-merged work** when `sync_main` sees overlapping paths (#730).
- **`lfd` SQLite health checks fixed** (#731); **native service environment now persists** across restarts (#734).
- **Loopflow's public website** is imported and deploys from the repo (#739).
- **Prompt trims** — loopflow operating guidance is opt-in and docs context is explicit rather than always-injected (#756, #760).
- Dependency bumps: `time` 0.3.51→0.3.53, `ignore` 0.4.26→0.4.27, `rusqlite` 0.39.0→0.40.1 (#769, #770, #771).

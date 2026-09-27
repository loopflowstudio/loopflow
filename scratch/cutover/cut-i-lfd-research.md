# lfd research (read-only, worktree `loopflow.data-model-one-table-per`, 2026-09-27)

Paths are relative to `rust/loopflow/src/` unless noted. "Observed" = read in
code/history. "Proposal" = my suggestion, not yet decided.

## 1. What lfd is and owns (observed)

`lfd` is a second binary (`Cargo.toml:22-24`, `bin/lfd.rs`, 319 prod lines) built
on `lfd/mod.rs` (1,591 prod lines before `#[cfg(test)]` at :1592),
`lfd/service.rs` (630), and `wave_host.rs` (457). Doc header `lfd/mod.rs:1-28`:
"the one process that must always be running on a Home machine ... It is *not*
a remote control API — reads become `lf` queries; hands become `lf` directly."

| Fact / job | Code | Owner today | Caller | Alternative without a resident process |
| --- | --- | --- | --- | --- |
| Wave listener hosting (tokio task per Wave, spawns `lf __resident`) | `wave_host.rs:194-355`, `lfd/mod.rs:1090-1094` | lfd in-process `WaveHost` | `lf start` → `POST /waves/start` (`lf/commands/home.rs:295`) | `lf wave serve <name>` foreground listener already exists (`bin/lf.rs:835`, `controller/wave/mod.rs:118-137`); pre-#1161 the Mac app launched it detached via tmux |
| Reconcile every 30 s (start placed+enabled Waves, stop unplaced) | `wave_host.rs:19,86-131` | lfd | none (loop) | launchd `KeepAlive` per Wave service, or none |
| `POST /waves/stop`, `/waves/reconcile` | `lfd/mod.rs:254-288` | lfd | `lf stop` (`home.rs:144`); reconcile route has **no caller in `lf`** (grep `waves/reconcile` → only router) | `lf stop` already has the fallback: `set_work_enabled(false)` + `request_stop` over the Wave's own `/stop` (`home.rs:150-151`) |
| PR landing claim + 30 s stale-landing recovery | `lfd/mod.rs:308-413`, `:1336-1362`; `pr_landing.rs:76-79` `LandingPlacement::Home` | lfd if it wins the claim | `lf pr land` → `ops/pr_landing.rs:913` (result ignored, `let _ =`) | already there: `wait_for_landing` claims `LandingPlacement::Local` and supervises in the CLI process (`ops/pr_landing.rs:946-966`); rerun `lf pr land` resumes (docs/architecture/delivery.md:171-173) |
| Linear webhook → durable inbox → `webhook::ingest_event` | `lfd/mod.rs:419-492`, `:609-691`; table `provider_deliveries` (`store/migrations/0.11.021_provider_deliveries.sql`, `store/sqlite/provider_deliveries.rs`) | lfd | Linear (external POST) | same domain op runs from polling: `refresh_task_comments` at every Task step start (`controller/task/mod.rs:252`) and every 15 s while a worker is attached (`:331-340`); both call `reconcile_linear_observation` (`ops/linear_observe.rs:95-105`, `webhook.rs:227-260`). The inbox table's only readers are lfd and `/status` |
| GitHub webhook (merged PR / deleted branch → prune worktree) + hook self-registration via `gh api` | `lfd/mod.rs:494-607`, `:736-767`, `:919-1032` | lfd | GitHub (needs public `LF_GITHUB_WEBHOOK_URL`; nothing in repo provisions a tunnel — grep `funnel|tailscale` in `rust/` = 0 hits) | the landing supervisor already observes the merge (`ops/pr_landing.rs:544+`) and could prune then; `lf wt prune` exists (`lf/commands/ops/mod.rs:1952`) |
| Maintenance sweep every ≥60 s: reap orphan OpenCode servers, auto-prune worktrees, prune terminal Task worktrees, prune abandoned prompt logs | `lfd/mod.rs:693-917`, gated by `config.autoprune` (`:1095-1100`) | lfd | none (loop) | `lf wt prune` (manual policy, `worktrees.rs:138-146`), resident startup already reaps OpenCode (`controller/wave/resident.rs:55`); a cron/`lf prune` |
| `GET /health` (home_id, store path, build, revision, migration frontier), `GET /status` (wave + delivery counts) | `lfd/mod.rs:214-238` | lfd | `install.rs` uses `/health` as keeper identity proof (`:1822,1878,1916,2217`); `/status` has no code caller | `machine_install::ArtifactIdentity` already captures binary identity (`machine_install.rs:1414`) |
| Home lock, endpoint file + bearer token | `~/.lf/lfd/<home>.lock`, `<home>.endpoint` (0600), `<home>.start.lock`, `startup/<attempt>.json` — `lfd/mod.rs:1453-1577` | lfd / `lf start` | `live_endpoint_at` (`:1396`) | goes away with the HTTP surface |
| Own log | `~/.lf/logs/lfd.log`, 8 MiB rotating (`bin/lfd.rs:12-118`) | lfd | — | — |

**How it is started — two lifecycles for one process (observed):**
1. Keeper: `lfd install` renders launchd plist / systemd unit (`lfd/service.rs:85-212,395-445`, label `com.loopflow.lfd`). Only `bin/lfd.rs:236-262` calls `service::install`; `lf install` does **not** install the service, it only pauses/resumes/rewrites an existing one (`install.rs:1801,1859,1868,2703-2712,3264-3284,3471-3482`). `scripts/install.py:505-520` promotes the binary via `--daemon-source/--daemon-target` but never registers the service.
2. Fallback: `lfd::ensure` (`lfd/mod.rs:1146-1307`) spawns `lfd serve --addr 127.0.0.1:0 --startup-attempt … --startup-receipt … --startup-socket …` in a tmux session `lfd-<home>` through `engine::process::start_home_session` (`:1252-1264`), then waits ≤10 s on a Unix socket for a receipt (`:1270-1306`, receipt code `:82-126`, `:1499-1535`). `docs/security.md:231-234` names this "the detached development fallback that `lf start` launches when no lfd service is live".
   Consequence: `install.rs:1798-1815` has to stop **both** (`service::pause()` then `tmux kill-session lfd-<home>`), and `bin/lfd.rs:155-170,215-234` carries three hidden flags only for path 2.

## 2. CLI-vs-client executor: verdict (observed)

**Gone.** Evidence:
- `#825` (`93960b439`) added `POST /v0/exec`, "the generic lf-argv exec door", explicitly "first increment of the lfd dissolution". `#872` (`309575f8e`, net −40,201 lines) deleted `src/lfd/**`, `src/lfdb/**`, `src/lfq.rs`: "The `lfd` daemon is gone: its HTTP service, route surface, SQLite catalog with 60+ accreted migrations, queue reconciler ... and the `lfq` remote-exec door". `git log -S'/v0/exec'` shows only #825 (add), `53a115534`, #872 (remove).
- `#1099` (`a7044e2b5`, −14,574): "delete Session and make Run the sole executor".
- The present `lfd` is a re-creation from `#1012` (`48f02ffd5`, +1,605, 2026-07) as a webhook inbox only ("deliberately not an API ... exactly three routes"), then grew Wave control in `#1119` (`9f91fc5d6`: "`lfd` is now the single Home server"), startup receipts in `#1161` (`cc3f7a44f`), landing claims in `#1231` (`3441ee5d4`), maintenance in `#1043`.
- Swift never touches lfd. `swift/Loopflow/Services/RegistryQuery.swift:87` runs `lf start <wave> --json`; `swift/LoopflowMac/MacLocalWaveAgentLauncher.swift:26-34,129` runs `lf stop`, `lf task start`. Only two Swift comments mention lfd (`MacLocalWaveAgentLauncher.swift:3`, `Views/WaveChatView.swift:455`). No `8080`, `/health`, `/waves` in `swift/`.
- The only HTTP clients of lfd are inside `lf` itself: `home.rs:144,190,295`, `install.rs` (health/pause/resume), `ops/pr_landing.rs:913`. No `SharedStore`-behind-HTTP proxy, no `LfdClient` type beyond the private `LfdClientEndpoint` record (`lfd/mod.rs:1386-1390`).
- "executor" in the tree today means only `SkillExecutor` (`engine/execution.rs:61`) and `CliFlowExecutor` (`lf/commands/flow.rs:500`) — one Flow driver, the H2 work on this branch. `ops/land.rs:249` "remote executor" refers to GitHub auto-merge.
- `wave/infrastructure/MEMORY.md:257-262`: "The summer moved from a remote daemon API to complete `lf` execution on each owning Home."

What *does* remain is not a CLI/client split but **same-operation-two-owners** pairs, listed in §4.

## 3. Does each job need a resident process? (observed + assessment)

| Job | Fact it owns | Without lfd | Lost | Code that could go |
| --- | --- | --- | --- | --- |
| Wave listeners | a live parent for `lf __resident`, the Wave HTTP server, Discord adapter | one detached `lf wave serve <name>` per Wave (tmux via `start_home_session`, or its own launchd unit) | one keeper that reconciles all repos on boot every 30 s; concurrent-caller sharing of a starting listener (`wave_host.rs:236-246`) | `wave_host.rs` (457), `/waves/*` routes + client (~250) |
| Landing claim | supervisor generation ownership across CLI exit | CLI process supervises (already the fallback); rerun resumes | landing continues after the terminal closes; auto-recovery of a crashed supervisor every 30 s | `lfd/mod.rs:308-413,1336-1362` (~135), `LandingPlacement::Home`, `recoverable_pr_landings` store fn |
| Linear webhook | inbox dedup + sub-second steer delivery | polling already exists at step start and every 15 s during a turn; edits/comments land through the same `reconcile_linear_observation` | ≤15 s latency; ingestion while **no** worker is attached (harmless — the next step-start refresh gets it); the `provider_deliveries` table | `lfd/mod.rs:415-492,609-691` (~160), `provider_deliveries` table + store (112 + sqlite impl), `webhook.rs` HTTP glue |
| GitHub webhook | prompt worktree prune on merge/branch delete | landing supervisor knows the merge; `lf wt prune`; sweep | latency on prune; nothing else | `lfd/mod.rs:494-607,736-767,919-1032` (~300) + `hmac`/`hex` use here |
| Maintenance sweep | periodic disk hygiene | `lf wt prune`, resident reap, or a cron entry | unattended disk recovery on `mini-heart` | `lfd/mod.rs:693-917` (~190), `config.autoprune` |
| Reconcile | placement → running | launchd `KeepAlive` per Wave unit; `lf start` at login | restart-after-crash of a listener; boot start across all repos | `wave_host.rs:86-131`, `/waves/reconcile` (no caller) |
| Health/status | keeper identity for install | `lfd --version`/artifact identity already captured by `machine_install` | nothing | `/status` now (no caller); `/health` with the daemon |

Remote Homes via Tailscale: unaffected either way — `lf ssh` is the remote
transport (`docs/architecture/homes.md:11-19`, `lf/commands/ssh.rs:1-23`); lfd
binds loopback and refuses non-loopback without `LF_LFD_ALLOW_NON_LOOPBACK=1`
(`lfd/mod.rs:1579-1588`). Offline: webhooks need a public ingress anyway; on
the laptop they cannot arrive, so polling is already the real path there.

## 4. Deletion / simplification candidates, ranked (proposals)

Sizes are production lines. "Counterexample" = the smallest observed thing
that would prove the candidate wrong.

1. **Second Linear receiver `lf pm webhook serve`** — `webhook.rs:295-349` (router/serve, ~55), `lf/commands/ops/mod.rs:914-952` (~40), `lf/mod.rs:1491-1509` (`PmWebhookCommand::Serve`). Same route `/linear/webhook`, same verify/parse/ingest, **no inbox dedup**, different default port 8899. Two receivers for one fact. Callers: none in code, none in `docs/*.md`. Breaks: nothing known. Counterexample: a Doppler/launchd config on a Home running `lf pm webhook serve` (check `launchctl list | grep loopflow`). Keep `Register`.
2. **Dual lfd lifecycle (tmux fallback beside launchd keeper)** — `lfd/mod.rs:82-126,1146-1307,1480-1535` (~330), `bin/lfd.rs:155-170,215-234` (~40), `install.rs:1804-1815` tmux kill, `engine/process.rs:380-410` `start_home_session_for_install_selection` (~30), `service.rs:239-393` install-switch rewriting exists only because a keeper plist must be re-pointed during promotion (~150). Pick one supervisor. Either (a) keeper only: `lf start` says "run `lfd install`" when no live endpoint, or (b) tmux only: delete `service.rs` entirely (630) and `KeeperMode`. Breaks: (a) `lf start` on a dev machine without the LaunchAgent (`tests/wave_start_tests.rs:133` exercises the fallback); (b) reboot persistence on `mini-heart`. Counterexample for (a): `wave/infrastructure/MEMORY.md:680-685` ("Supported Wave startup is one event-driven Home lifecycle", decided 2026-07-21) which specifies the receipt/socket wake — that decision would need re-opening.
3. **GitHub webhook + `gh api` hook registration** — `lfd/mod.rs:494-607,736-767,919-1032` (~300) plus `GithubConfig`, `LF_GITHUB_WEBHOOK_*` scrub entries (`engine/process.rs:504,556-557`), `docs/lf.md:1280-1285`. It runs `doppler secrets get --plain` in-process (`lfd/mod.rs:925-928`) and pipes the secret into `gh api` (`:989-1002`). Needs a public URL the repo never provisions. Breaks: prompt prune on merge if a hook is actually registered. Counterexample: `gh api repos/loopflowstudio/loopflow/hooks` showing a hook whose `config.url` equals the Doppler `LF_GITHUB_WEBHOOK_URL` (inspect with `doppler secrets --only-names` first).
4. **Landing claim route + recovery loop** — `lfd/mod.rs:308-413,1336-1362` (~135), `LandingPlacement::Home` (`pr_landing.rs:76-96`), `store` `recoverable_pr_landings`, and the `Home` arm in `store/sqlite/pr_landings.rs:36-50`. The CLI already supervises when lfd says no (`ops/pr_landing.rs:913` ignores the result). Breaks: landing dies with the terminal; no auto-recovery of a crashed supervisor. Counterexample: Jack relying on closing the laptop lid during `lf pr land` on `mini-heart` — but lfd on the *same* machine dies with it; only a remote Home benefits. If detachment is wanted, `start_home_session("lf-land-<id>", …)` gives it without HTTP.
5. **Maintenance sweep** — `lfd/mod.rs:693-917` (~190), `autoprune` config. Duplicates `lf wt prune` policy with `automatic()` instead of `manual()` and duplicates `resident.rs:55` OpenCode reaping. Breaks: unattended disk recovery (`#1043`'s motivation: "worktrees: recover disk automatically"). Counterexample: disk pressure incidents on `mini-heart` between manual prunes. Cheapest keep: run the same sweep once at the end of `lf pr land` and `lf start`.
6. **Linear inbox → polling only** — `lfd/mod.rs:415-492,609-691` (~160), `provider_deliveries` (table + `store/provider_deliveries.rs` 112 + `store/sqlite/provider_deliveries.rs`), `tests/lfd_tests.rs`. Breaks: ≤15 s steer latency. Counterexample: `#1275` ("steering: route task direction through linear comments") lists webhooks *and* polling as ingest paths; if Jack wants sub-second Discord-style steering, the webhook stays and then a resident process is unavoidable.
7. **Whole lfd → per-Wave detached `lf wave serve` under launchd/tmux** — `lfd/` (2,540) + `wave_host.rs` (457) + `bin/lfd.rs` (319) + `ArtifactRole::Daemon` plumbing in `machine_install.rs` (:128,186,554-555,697,750,1123) + package/release (`.github/workflows/package-build.yml:54-74`, `scripts/install.py:84,263,505-520`, `scripts/check_architecture.py:33,223`) + docs (`architecture-reference.md:283,305,310,414,744-780`, `architecture/homes.md`, `codebase.md`, `lf.md:359-363,650-660,1280`, `security.md:222-240`). Follows only if 1–6 land; it is the size of #1012+#1119+#1161 combined (~5,800 insertions).
8. **`/waves/reconcile` route and `/status` route** — no callers anywhere (`grep waves/reconcile` → router only; `delivery_count` → `/status` + tests). ~35 lines. No counterexample beyond curl by hand.

Not a candidate: `lfd/mod.rs:1579-1588` bind guard and control-token auth stay
as long as any route stays.

## 5. `start_home_session` and process owners (observed)

`engine/process.rs:363-378` → `start_session_with_context` → `start_tmux_session`
(`:417-445`). It is **tmux**, not lfd and not launchd: it launches a detached
`lf`/`lfd` process with the current Home control pair pinned in env
(`LF_CONTROL_BIN/HOME/DB_PATH`, `:447-470`). Callers: `ops/flow_run.rs:82`
(saved Flow driver, this branch), `ops/human_session.rs:1698`, and lfd's own
fallback (`lfd/mod.rs:1263`). So today there are four process owners on a Home:
launchd/systemd (keeps lfd), tmux (detached lf drivers + fallback lfd), lfd
tokio tasks (Wave listeners), and the listener (spawns `lf __resident`,
`controller/wave/mod.rs:217-259`). None supervise each other; only lfd
reconciles. `docs/architecture/homes.md:73-79`: "The process that directly
spawns a child owns that child handle."

## 6. Stated intent (observed)

- `docs/architecture/homes.md:112-116`: "`lfd` serves one Home. It reconciles eligible Wave listeners, receives Linear and GitHub webhooks, and claims PR landing work."
- `docs/architecture-reference.md:310`: owner column "`lfd` starts eligible Wave listeners; the promotion command owns only its OS-locked switch transaction".
- `docs/security.md:231-234`: tmux launch is a "development fallback"; "Install lfd as the Home service for durable webhook ingress."
- `wave/infrastructure/MEMORY.md:680-689` (decided 2026-07-21): one event-driven startup lifecycle with receipt + socket; "Reconciliation polling remains recovery, never startup acknowledgement."
- `wave/infrastructure/GOAL.md`: no mention of lfd, daemon, keeper or webhooks.
- `docs/concept-review.md:43,58`: #872's deletion of lfd is held up as the model example of "clearer product meaning → simpler representation → simpler infrastructure".

## 7. Decisions for Jack (exact choices)

A. Wave residency: (1) keep one Home keeper (`lfd` + `WaveHost`), or (2) one detached `lf wave serve <name>` per Wave, kept alive by launchd/tmux, no daemon. (2) removes ~3,300 lines and the second binary; loses the 30 s reconcile and shared startup between concurrent `lf start` callers.
B. lfd lifecycle, if A=(1): (1) launchd/systemd keeper only, `lf start` errors "run `lfd install`" — deletes the receipt/socket/tmux fallback (~400); or (2) tmux fallback only — deletes `service.rs` (630) and `KeeperMode`.
C. Linear ingress: (1) webhook + inbox (needs a resident process and public URL), or (2) polling only (15 s during turns, step-start otherwise). Either way delete `lf pm webhook serve`.
D. GitHub webhook: keep only if a hook is registered today; otherwise delete and prune at landing completion.
E. PR landing detachment: (1) CLI-only supervision (delete Home claim + recovery), or (2) keep detach by spawning `lf pr __supervise` via tmux, no HTTP.

## 8. Smallest next action

Delete `lf pm webhook serve` (`webhook.rs:295-349`, `lf/commands/ops/mod.rs:914-952` Serve arm, `lf/mod.rs:1496-1501`), keeping `Register`, `verify_signature`, `parse_event`, `ingest_event`. It is a strict duplicate of lfd's route without the inbox, has no callers or docs, and needs no product decision. Then decide A/B, since every other row's shape depends on whether a resident keeper exists.

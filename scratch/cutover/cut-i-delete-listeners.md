# Cut I — delete Wave listeners, residents, and lfd (LOO-298)

Jack's decision, 2026-09-27: "delete `lf wave serve` and every notion of a Wave
listener or resident, lfd included." One implement pass on this branch. Paths
are relative to `rust/loopflow/src/` unless noted; "prod" = lines before the
tests module. Observed facts carry file:line; anything else is the proposal.

## 1. Target: what remains and who owns it

| Fact / operation | Owner after the cut | Evidence it is already independent of a resident |
| --- | --- | --- |
| Wave record (id, repo, slug, chapters, retirement) | `waves`, `wave_chapters` tables; `work/wave/`, `ops/chapter.rs`; `ensure_wave_row` moves from `controller/wave/registry.rs:43-101` into `work/wave/` | `ops/pm.rs:909,1039`, `ops/chapter.rs:385`, `home.rs:233-247` call it without a listener |
| GOAL.md / MEMORY.md / metric instruments | authored files; `metric_instruments`, `metric_observations` (`store/metrics.rs`, `store/sqlite/metrics.rs` 681); `controller/wave/metrics.rs` (1,176 prod) **moves** to `work/wave/metrics.rs` | readers are `ops/metrics.rs:8`, `work/chapter.rs:79,86`, `pm/mod.rs:79`, `lf/commands/waves.rs:23`, `tests/status_tests.rs:13`, `tests/dto_fixtures.rs:1` — none of them listener code |
| Wave operation (`wave/operate`) | a finite Run: `lf --wave <w> wave/operate` (docs/lf.md:364) or a cron | `docs/architecture/planning.md:217-224`: "one finite wave/operate Run … The Wave listener and resident are not prerequisites" |
| Crons | `lf cron sync/run` + launchd (`ops/cron.rs`, 1,105 prod). The plist runs `lf cron run --scheduled --wave W --flow F` (`:936-945`), which spawns `lf --wave W --batch flow|skill F` (`:771-784`) | no resident involved; `lf doctor` audits obligations (`lf/commands/doctor.rs:99`) |
| Projects / Tasks / Task workers | `controller/task/`, `lf task run|restart|resume`, `lf task __worker` | Linear polling at every step (`controller/task/mod.rs:252`) and every 15 s during a turn (`:331-340`) |
| Task steering | Linear comments, `lf task steer` → `ops/linear_observe.rs:46,95` | same domain op the webhook fed (`reconcile_linear_observation`) |
| PR landing | the invoking `lf pr land` process (`ops/pr_landing.rs:946-966`, `LandingPlacement::Local`); rerun resumes (docs/architecture/delivery.md:171-173) | already the fallback when lfd is absent (`:913` ignores the claim result) |
| Home identity, placement, remote transport | `homes`, `work_placements`; `lf home id|observe`, `lf work place`, `lf ssh` (`lf/commands/ssh.rs`), `HomeRoute` (`engine/wave_home.rs:59-215`) | placement is used by `ops/cron.rs`, `controller/task`, `install.rs` |
| Command journal (`lf ps|top|prune`) | `journal/mod.rs` (915 prod) — the *command* ledger, not the Wave journal | `tests/store_contention.rs:4` refers to `journal::open_ledger` |
| Install / promotion of `lf` + app | `machine_install.rs`, `lf install` minus every Daemon role | — |
| Harness conversation types | `chat/types.rs` (286 prod, `ConversationEvent`) — 14 non-listener users (harness, run_record, controller/task) | keep; fold `chat/mod.rs` to it |

Nothing resident survives: no HTTP server in `lf`, no second binary, no
tmux/launchd keeper, no Discord, no Wave journal, no `paused` turn intent.

## 2. Ordered deletion list (by module)

Order chosen so each step compiles alone. Sizes are prod lines removed.

### I.1 lfd binary, keeper, WaveHost — 2,997
- `bin/lfd.rs` (319), `lfd/mod.rs` (1,591), `lfd/service.rs` (630), `wave_host.rs` (457); `lib.rs:11,32`; `Cargo.toml:22-24` `[[bin]] lfd`.
- Callers to rewrite: `lf/commands/home.rs` (`start`/`stop`/`start_inner`, `:107-330`), `lf/commands/install.rs` daemon paths (below), `ops/pr_landing.rs:913`.
- `engine/process.rs:106-160` `resolve_lfd_binary*`/`select_lfd_binary` (+ tests), `:380-410` `start_home_session_for_install_selection`, scrub list entries `LF_LINEAR_WEBHOOK_SECRET LF_LINEAR_VIEWER_ID LF_GITHUB_WEBHOOK_SECRET LF_GITHUB_WEBHOOK_URL LF_LFD_ALLOW_NON_LOOPBACK LF_DISCORD_TOKEN` (`:504,556-557`, `:7`).
- Files/locks that stop existing: `~/.lf/lfd/*.endpoint|*.lock|*.start.lock|startup/`, `~/.lf/logs/lfd.log`, `~/Library/LaunchAgents/com.loopflow.lfd.plist`, tmux session `lfd-<home>`.
- Tests: `tests/lfd_tests.rs` (278), `tests/wave_start_tests.rs` (356), in-file tests (lfd 508, service 140, wave_host 303).
- Docs: `docs/security.md:222-240`; `docs/lf.md:359-363,650-660,1280-1285`; `README.md:73-78`.

### I.2 Webhook receivers and the delivery inbox — ~560
- `webhook.rs` (186 prod; whole file — `verify_signature/parse_event/ingest_event` have no caller once lfd and `lf pm webhook serve` go), `store/provider_deliveries.rs` (112), `store/sqlite/provider_deliveries.rs` (88), `store/mod.rs` wrappers `record_delivery/complete_delivery/delivery_count`, `store/sqlite.rs` glue.
- `lf pm webhook serve|register`: `lf/mod.rs:1491-1509`, `lf/commands/ops/mod.rs:909-952`; `pm/linear.rs` `create_webhook` if unused after.
- Forward migration draft dropping `provider_deliveries` (created `0.11.021`).
- Tests: `tests/linear_webhook_tests.rs` (132); `tests/linear_observe_tests.rs:12-25` comments only.
- Cargo: `hmac` (0 other users), `tower-http` (0).

### I.3 Wave listener, server, resident, runner, journal, Discord — ~9,230 (+ ~150 in relocate)
- Delete: `controller/wave/{mod.rs 285, channel 75, chat 145, chat_reply 104, discord 1081, journal 1540, playhead 77, recovery 182, resident 391, runner 1154, runtime 1632, server 1242, state 72, subscription 92, supervisor 504, wire 177, placement 187}`, `controller/wave/README.md` (155), `registry.rs` minus `ensure_wave_row*` (~290 of 352).
- Keep and move: `controller/wave/metrics.rs` → `work/wave/metrics.rs`; `registry::ensure_wave_row*` → `work/wave/`; `controller/wave/relocate.rs` → `work/wave/relocate.rs` minus `WaveLocatorLock` (`:32-60`, only other users are `recovery.rs:155` and `mod.rs:323`), `ensure_no_live_endpoints` (`:510-530`), journal moves (`:474-475,532-549,724-725`). `controller/mod.rs` keeps `task` only.
- `chat/turns.rs` (418) — users outside the listener are only `lf/commands/thread.rs:29` and `playhead.rs:3`; delete. Keep `chat/types.rs`.
- `lf __resident`: `lf/mod.rs:392`, `bin/lf.rs:1463`; `lf wave serve|recover`: `lf/mod.rs:923-940`, `bin/lf.rs:835-858`.
- Env: `LF_WAVE_SERVER_ENDPOINT`, `RESIDENT_TOKEN_ENV`. Keep `LF_WAVE_ID` (Task attribution).
- Observation outbox (writer with no reader once `registry::StoreObserver` goes): `store/children.rs:553-575` `pending_observations`/`pending_project_observations`/`mark_observation_delivered` (the Project reader never existed — grep shows none), `store/sqlite/children.rs:1089-1135,2155`, `child.rs:72-75` `ObservationRecipient`, table `observation_outbox` (`0.10.001_initial.sql`) → forward migration.
- Wave journal files `.lf/journal/waves/<name>/journal.jsonl`: no reader remains. Leave on disk (gitignored, per-machine); no importer, no deletion command.
- Bench: `benches/wave_stream.rs` (136), `Cargo.toml` `[[bench]]`.
- Cargo: `tokio-tungstenite` (0 other users). `axum` stays for now (`pm/`, `ops/pm/*` tests, `ops/linear_observe.rs`, `controller/task/mod.rs`) — check whether every remaining use is `#[cfg(test)]`; if so move to dev-dependencies.
- Tests: `tests/wave_journal_tests.rs` (546); `tests/wave_repository_ownership.rs:4-5,48` (journal path + relocate — rewrite to relocate-only); `tests/context_tests.rs:666` (one test); in-file tests ≈ 7,500 (runtime 1,440, discord 1,586, runner 1,056, journal 873, server 465, supervisor 392, others).
- Builtins: delete `engine/builtins/wave/skill/wave_chat.md` (43); edit `wave/skill/scan.md:43,113` ("resident state"), `wave/skill/wave_operate.md:35`, `ops/skill/init.md:246,280`, `ops/skill/loopflow.md:143` (`lf start`).
- Docs: `docs/waves.md:7,30-39,124-145,207-283` (Discord chat section), `docs/lf.md:323-327,365,377`, `docs/architecture/planning.md:220-224,257-259`, `docs/architecture/data.md:23,130`, `docs/architecture-reference.md:301,331,368,415,471,614,704,744-746,765-768,794,831,876`, `docs/architecture/codebase.md:27,54,63,115-118`, `docs/getting-started.md:20,171-179,201,221,231`, `docs/troubleshooting.md:8-17`, `docs/config.md:36-39`, `docs/agent-api.md:24,108`, `docs/conducting.md:101`, `TESTING.md:283,474,527`.
- `scripts/check_architecture.py:32-33,223` (`WAVE_SERVER`, `LFD_SERVER` route discovery) and `python/tests/test_architecture.py:38,82,194`.

### I.4 CLI lifecycle and chat commands — ~1,900
- Die outright: `lf start`, `lf stop` (`lf/mod.rs:360-374`, `home.rs` ~400 of 479; keep `id_cmd`/`observe_cmd` `:17-72`), `lf pause`, `lf resume` (`lf/mod.rs:375-390`, `lf/commands/wave_intent.rs` 46, `work/wave/config.rs:72-76,320-340` `paused`/`update_wave_paused`), `lf chat` (`lf/commands/chat.rs` 562, `thread.rs` 311), `lf reply` (`reply.rs` 35 — it is the `wave/chat` skill with stdin; a person runs `lf wave/chat : "…"` if they ever want a one-shot, so no bounded-Run replacement is needed), `lf home probe` (`lf/mod.rs:1584-1592`, `home.rs:74-105`, `ops/home.rs` 126, `engine/wave_home.rs:217-287` `HomeState/HomeActionDto/HomeRuntimeDto`; keep `:23-215`), `lf/commands/fixtures.rs` (61), `bin/lf.rs:1447-1465,1612-1643`.
- `lf ls` / `lf status`: `lf/commands/waves.rs:27,45-100,568-593,819-870,1582-1610,1651,1697-1733` — drop `live`, `paused`, `endpoint`, `loop_state`, `home_runtime`; keep `enabled` (it is the `lf work enable|disable` control for every Work kind, docs/lf.md:663-666).
- `work/wave/config.rs:49-71` `WaveChatConfig`/`chat:` key (Discord binding) and `:114` reader.
- DTO fixtures `tests/fixtures/dto/`: delete `chat_history, chat_turn, post_message_error_response, post_message_response, resident_deltas, resident_door, turn_delta, wave_attempt_messages, wave_loop_states`; edit `wave_detail.json` (`live/paused/endpoint/loop_state/home_runtime`), `README.md:7-9`; `tests/dto_fixtures.rs`, `swift/LoopflowTests/DTOFixtureTests.swift:395`.

### I.5 PR landing Home claim — ~60
- `pr_landing.rs:76-96` `LandingPlacement::Home` (keep `Local`, or drop the enum to a unit), `store` `recoverable_pr_landings`, `store/sqlite/pr_landings.rs:36-50` Home arm; `ops/pr_landing.rs:913`.
- Docs: `docs/architecture-reference.md:305` owner column, `docs/architecture/delivery.md:174`.

### I.6 Install, promotion, packaging — ~470 Rust + scripts
- `machine_install.rs:38-43` `ArtifactRole::Daemon`, `:128,186,554-555,697,750,1123` and `required_machine_artifact_roles`.
- `lf/commands/install.rs`: `PausedHome` (`:224-228`), `artifact_set_digest` daemon arg (`:907`), `stage_daemon_binary` (`:959`), `preserve_prior_daemon` (`:1028`), `validate_daemon_candidate` (`:1477`), `retained_daemon_path` (`:1545`), `pause_home`/`resume_home_*`/`verify_switch_home` (`:1798-1930`), `active_keeper_matches` (`:2202-2230`), switch calls `:2703-2712,3264-3284,3471-3482`; `lf/mod.rs:1193-1225` `--daemon-source/--daemon-target`; `install/published.rs:90,144,303,353,359`; `doctor.rs:361` `AppHelper("lfd")`.
- `scripts/install.py:84,263,470,486,514-517`; `scripts/publish_release.py:144-149,317`; `scripts/release-loopflow.py:159,172`; `scripts/loopflow-dev.py:605,615,695`; `.github/workflows/package-build.yml:54-74`; `tests/e2e/install_bootstrap.py:3,30,87,238-242,262,290`; `python/tests/test_{install_script,release_publisher,release_loopflow,shell_installer}.py` lfd assertions; `swift/project.yml:58`; `swift/README.md:294`; `TESTING.md:474`; `docs/lf.md:1197-1210`.

### I.7 Swift app — ~3,200 prod, ~1,300 tests
- Delete: `Services/WaveChatClient.swift` (874), `Views/WaveChatView.swift` (818), `Views/WaveChatRendering.swift` (294), `Views/MessageRow.swift` (217), `ConversationLaunch.swift` (66; `SessionsView.swift:511` caller), `Models/ChatTurn.swift` (347), `Models/ConversationTypes.swift` (113), `Models/WaveChatTranscript.swift` (99), `Models/ChatReference.swift` (259 — verify no non-chat reader), `MockWaveFixture.swift` chat parts (467 total).
- Edit: `Services/RegistryQuery.swift:76-110` (`probeHome`, `start`, `setWavePaused`), `:370-430` (`HomeState`, `HomeRuntime`, `WaveSnapshot.live/paused/endpoint`), `Models/Wave.swift:14-15`; `MacLocalWaveAgentLauncher.swift:3-4,27-35` (`stopWave`); `PodiumModel.swift:405-423` (`setWavePaused`); `Views/RoadmapView.swift:528-600,696` (`HomeRuntime` attach/start action, `wave.live` dot); `Views/WaveChatView` comment references in `WaveChatView.swift:455`.
- Tests: delete `WaveChatConnectionTests` (588), `WaveChatStreamTests` (115), `WaveChatTranscriptTests` (248), `MessageRowTests` (51), `ChatReferenceTests` (218), `BundledDaemonPathTests` (32); edit `RegistryQueryTests`, `ContractTests`, `PodiumModelTests` (742), `LocalWaveAgentLauncherTests` (172), `FaderSwitchTests`/`PodiumStateTests` per §5 Q2.
- `swift/README.md:192-193,229`.

**Rough total:** Rust ≈ 16,000 production lines (+ ≈ 9,000 test lines), Swift ≈ 3,200 (+ ≈ 1,300), plus ≈ 150 lines across Python scripts, workflows, fixtures, and ≈ 60 doc paragraphs. Net after moves (metrics, ensure_wave_row, relocate trim): ≈ 15,500 Rust production lines gone.

## 3. What changes for a person (plainly)

- **Talking to a Wave.** `lf chat`, Discord chat, and the Mac Wave Chat pane are gone. To steer a *Task*: comment on the Linear issue or `lf task steer` — the worker reads it at the next step and every 15 s during a turn (`controller/task/mod.rs:252,331-340`). To steer a *Wave*: edit `GOAL.md`/`MEMORY.md` and run `lf --wave <w> wave/operate`, or let its cron fire. The old journal at `.lf/journal/waves/<name>/journal.jsonl` stays on disk with no reader; nothing imports it.
- **Wave "running".** Nothing runs between operations. `lf start`, `lf stop`, `lf pause`, `lf resume`, `lf wave serve` are gone; startup on boot is gone. Recurring work is `crons:` in GOAL.md + `lf cron sync --wave <w>` on the placed Home — launchd fires `lf --wave <w> --batch flow <f>` (`ops/cron.rs:771-784`) with no resident. The resident's in-process cron firing (`controller/wave/runner.rs:26-28,209-240`) was a second owner of the same schedule; it goes.
- **`lf ls` / `lf status`.** No `live`, `paused`, `endpoint`, `loop_state`, `home_runtime`. Chapter, Tasks, metric portfolio, Runs remain.
- **Metric instruments.** Unchanged; `lf metrics` and the chapter portfolio keep working from `metric_*` tables.
- **PR landing.** `lf pr land` supervises in its own process. Closing the terminal stops supervision; rerun `lf pr land` to resume (docs/architecture/delivery.md:171). No background recovery of a crashed supervisor.
- **Worktree pruning.** Only `lf wt prune` (manual policy, `engine/worktrees.rs:138-142`). The 15-minute sweep, merged-PR webhook prune, and OpenCode orphan reaping at sweep are gone; the resident startup reap (`controller/wave/resident.rs:55`) goes with the resident, so orphan OpenCode servers are reaped only by `lf prune`.
- **Linear/GitHub webhooks.** Gone, including `lf pm webhook register`. Steering latency is ≤15 s during a turn, next step start otherwise.
- **Mac app.** Wave Chat pane, fader on/off (start/stop), pause toggle, "attach/start" action, and live dot disappear; Task workspace, sessions, runs, roadmap, metrics remain. The app stops bundling `lfd`.
- **Install.** `lf install` promotes `lf` and the app only. Homes that already have `com.loopflow.lfd` loaded must run the *old* `lfd uninstall` before the new binary lands, or launchd keeps relaunching a missing program.

## 4. Smallest counterexample per group

- I.1: a Home where `launchctl list | grep com.loopflow.lfd` is loaded and someone expects Waves to come back after reboot.
- I.2: `gh api repos/loopflowstudio/loopflow/hooks` (or the Linear webhook list) showing a hook pointed at a Home — the receiver disappears and the provider retries into a closed port. (Check names only: `doppler secrets --only-names` for `LF_GITHUB_WEBHOOK_URL`, `LF_LINEAR_WEBHOOK_SECRET`.)
- I.3: a GOAL.md with `chat: {provider: discord, …}` (docs/waves.md:207) whose channel people still use; or a `crons:` line that has *not* been synced with `lf cron sync` and was firing only through the resident.
- I.4: a script or the Mac app calling `lf start --json` / `lf home probe --json` (`swift/Loopflow/Services/RegistryQuery.swift:81,87`) — this pass edits those callers.
- I.5: a landing on `mini-heart` that someone expects to finish after the SSH session that ran `lf pr land` drops.
- I.6: `tests/e2e/install_bootstrap.py:238-242` asserting `lfd --version` — rewritten in this pass.
- I.7: `ChatReference.swift` having a non-chat reader (grep before deleting).

## 5. Product questions only Jack can answer

1. **`paused`.** Delete the GOAL.md `paused` key and `lf pause|resume` (my recommendation: only the resident read it — `controller/wave/runtime.rs:702`, `server.rs:422`, `waves.rs:823`; `lf cron run` never does), or keep `paused` as a gate that `lf cron run` and `wave/operate` honour? Exact choice: **delete** vs **re-point to cron gate** (≈30 lines added in `ops/cron.rs:416-450`).
2. **Podium fader.** With no start/stop, the fader either (a) is removed with `FaderSwitch.swift`/`FaderSwitchTests`/`PodiumStateTests`, or (b) becomes `lf work enable|disable wave <id>` (the surviving placement control, `store.set_work_enabled`). Exact choice: **remove** vs **rebind to enable/disable**.

## 6. Proof for the pass

Suites (crate-wide deletions, so the affected suite is the crate):
- `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test -p loopflow` (full, once).
- `uv run pytest python/tests/test_architecture.py python/tests/test_install_script.py python/tests/test_release_publisher.py python/tests/test_release_loopflow.py python/tests/test_shell_installer.py`; `uv run python scripts/check_architecture.py`.
- `swift test --package-path swift --no-parallel -Xswiftc -gnone` (as `.github/workflows/ci.yml:146`).
- Fixture round-trip: `cargo test -p loopflow --test dto_fixtures` and `DTOFixtureTests`.

Manual, on a private Home (`LF_DB_PATH=<copy>`; per memory, dev `lf` refuses the production DB):
1. `lfd uninstall` with the *old* binary, `launchctl list | grep loopflow`, `tmux ls | grep lfd-` → nothing loaded.
2. `lf ls --json` and `lf status <wave> --json` → no `live/paused/endpoint/loop_state/home_runtime`; metric portfolio present.
3. `lf --wave <wave> --batch flow wave/operate` (or the Wave's own flow) → one finite Run in `lf runs`.
4. `lf cron sync --wave <wave>`; `lf cron list`; `lf cron run --wave <wave> --flow <f>` → receipt in `lf cron history`.
5. `lf task run <issue>` through one step; post a Linear comment mid-turn → appears as a Steer within 15 s.
6. `lf pr land` on a throwaway PR → merges in-process; Ctrl-C then rerun resumes.
7. `lf install promote --from-build … --preview` without `--daemon-*` → plan names `lf` and app only.
8. Mac app: open a Wave → roadmap, Tasks, runs render; no chat pane, no fader/pause.

## 7. Order of work

I.1 → I.2 → I.5 (small, unblocks the build) → I.3 (largest; move `metrics.rs`, `ensure_wave_row`, `relocate.rs` first, then delete `controller/wave/`) → I.4 → I.6 → I.7 → docs and fixtures → forward migrations (`provider_deliveries`, `observation_outbox`) → proof. Commit per group so each step is reviewable and revertable.

## Discord (Jack, 2026-09-27)

"I don't want to delete the discord integration, but I want to move that to be
an independent thing on top of other foundations, not assume some other idea of
a wave resident. Its exact behavior is not super important and will require
future followup, though better if it still works in some basic form for now."

So I.3 does not delete the Discord client. It moves it out of the Wave listener
into an independent bridge that owns nothing about Waves: a channel message
becomes one bounded Run (`lf --wave <mapped wave> : "<message>"` or the Wave's
`wave/operate` with the message as input), and the Run's final answer is posted
back to the channel. Configuration maps channel → Wave in `.lf/config.yaml` or
GOAL.md frontmatter as today. Whether the bridge is a cron-fired poll or a small
standalone `lf discord serve` process is the implementer's call; either is
independent of any resident and must not reintroduce a Wave HTTP server, journal
reader or inbox. Basic send/receive is the bar; anything beyond is follow-up.

## Implementation ledger — 2026-09-27, in progress

Cut start: `5eec3a805b78003e06ba483b0668769e3bfd8fd6`; main comparison:
`5bdcef6b65419d5db2583ee791cede2f1a95b3df`. This ledger supersedes the
original deletion list where it conflicts with Jack's final Discord correction.

- Removed the lfd binary, keeper and WaveHost, webhook receiver/registration,
  Home landing handoff, Wave controller directory and chat/lifecycle commands.
  Metrics and relocation moved to `work/wave`; the command journal and Task/Flow
  executor remain. Relocation still protects unmerged authored checkout bytes
  and excludes historical boot files; it no longer moves the old Wave journal.
- Chosen bridge: `lf discord serve <wave>` polls the existing GOAL.md channel
  binding. Each non-bot message launches one finite attributed Run through the
  ordinary launch and settlement path; only its final answer is posted, with
  mentions disabled. Cursor is in memory; startup skips old messages. No live
  Discord request or provider execution occurred during implementation.
- Implementation choices, not new Jack approval: delete resident-only paused
  and fader controls. Work enablement stays. Retain `ConversationLaunch.swift`
  and `ChatReference.swift`: Task/Session launch and ReferenceTextView still use
  them. Delete chat-specific conversation/rendering types and panes.
- New installs select CLI plus optional app. Retained old artifact sets still
  decode the historical `daemon` role and verify their original digest; that
  role cannot create an entry gate or launch a service. Historical switch input
  keeps an optional daemon target solely for receipt identity. No old artifacts,
  installed selection or OS service was changed.
- New draft `drop_wave_services` drops the receiver inbox/outbox and translates
  Home supervisor placement to local while retaining PID/heartbeat/generation.
  Existing exact process and lock recovery still determines takeover authority.

Resource preflight passed twice, latest 67.6 GiB free / 64 GiB floor. Source
checks scrub LF_*/LOOPFLOW_*, use nice +10, four workers and a 900-second limit.
Rust library and CLI compilation passed before fixture conversion. First
all-target compilation identifies obsolete lifecycle/outbox tests; conversion
is in progress. No behavioral pass, migration rehearsal, Swift pass, configured
provider, installed Home or rendered app acceptance is claimed yet. A stray
`#[cfg(test)]` left by fixture-module deletion initially hid the shared Flow
module; removed it without changing H3 executor logic. Logs: `.lf/tmp/cut-i/`.

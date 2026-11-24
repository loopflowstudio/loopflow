# LOO-1081 · Run a Task on Modal

**Done when** a Linear-backed Task runs its Graph on a fresh Modal Machine with the caller’s portable settings and a usable harness. An idle sandbox pays the small resource floor while quotes and caps cover peak compute. Giving it back needs neither a manual Git commit nor a database edit, and ended rentals settle cleanly without cluttering live Machines. Session history, transcripts and recorded usage remain available across work-scoped analytics (`lf session usage`, `lf session history search`, and Desktop history/telemetry views) after abrupt loss.

## Model

1. **Adopting an authored Wave is separate from authoring a new one.** Cold acquisition on a fresh clone registers existing `.lf/wave/` files without writing or committing to `main` and without inventing a local `tracker_id: null` Project. It binds the sole In Progress Linear Project from a complete Initiative inventory; explicit `lf wave plan bind` handles ambiguous or misselected Waves.
2. **Placing an unowned Task on a Machine is one atomic ownership entry.** Never claim locally first and move second. Write the target owner only after the remote checkout and server are ready.
3. **Modal sizing separates burst ceiling from idle floor.** `--vcpus` and `--memory-gb` are burst ceilings with Modal’s minimum floor (`0.125` physical core, `128 MiB`) unless `--reserved` pins floor = ceiling. Admission, caps, and timeouts price the ceiling; uptime-derived cost is explicitly labeled as a ceiling estimate.
4. **Container Linux is an ordinary Machine.** Missing systemd or cron never blocks `lf server connect` or `lf install`. Rentals receive the caller’s matching Linux `lf` build before remote GitHub login exists.
5. **Execution adapts to the Machine’s installed harnesses; every server-started Session reports unavailability truthfully.** Personal preferences fall back to usable installed native harnesses on the execution Machine (`-a` stays exact; established threads keep their identity). A missing executable or login leaves recovery quiescent until inputs change, and every server-initiated start (Task recovery, steer wakeup, and `lf machine forward` restart) shares one Session readiness check and `start_failure` surface.
6. **Ended rentals are history, and their collected records feed the same analytics as local work.** `lf machine end` checks the provider before warning about unpushed work, so an already-terminated sandbox settles without `--force`. Ended rentals remain in SQLite for `from <label>` attribution and `lf ondemand list` history, not as dead routes in `lf machine list`. Collected rental records in `machine_records` feed work-scoped `lf session usage`, `lf session history search`, and `SessionHistory` views (tagged with their origin Machine) rather than living behind a single-Session side flag alone.

## 1. Built — admit one usable Modal Machine

- **Modal burst sizing and ceiling estimates.** Admission records the sizing mode in the rental’s hardware. Old declarations acquire burst limits on their next admission. A rental with no recorded limits retains its historical hardware and cost. New Modal rentals expose `ceiling_estimate_usd` in CLI JSON.
  - For 8 vCPU / 16 GiB, the pinned Modal 1.6.2 SDK sends `125/4000` millicores and `128/16384` MiB in burst mode, and `4000/4000` and `16384/16384` with `--reserved`. Both price the $0.95184/h ceiling at admission. Modal’s timeout takes remaining whole seconds at create.
  - Jack Heart’s billing steer: “Billing data point: actual Modal dashboard usage was $1.65 vs ~$1.78 from lf fixed uptime math ($0.95184/h across modal-1..6), confirming lf currently sends --vcpus/--memory-gb as a full request floor rather than burst-only limits.”
- **Caller-resolved Linux packages.** Release callers fetch their tagged Linux archive and verify its SHA-256. Main packages carry the GitHub run ID that produced them; clean source builds can also look up packages by commit. `LF_RENT_LINUX_BIN` installs an unpublished build into `~/.local/libexec/lf` with a `~/.local/bin/lf` wrapper (no `sudo`) targeting `~/.lf-dev`. The shared image includes Codex and Claude Code and puts `/home/lf/.local/bin` on the non-login SSH path.
- **Container installation and server readiness.** Without a systemd user bus, the selected CLI starts a detached `lf server run` under the install lock and retires superseded detached servers safely via `SO_PEERCRED` + `pidfd_open`. Missing `crontab` leaves automatic updates unscheduled during `lf install`, while `lf install schedule` still reports it missing. Promotion inspects pre-migration v0.15.1 stores without forcing post-migration indexes.

## 2. Built — adopt the existing Wave and hand off atomically

- Cold registration reads Wave identity from authored files without modifying `.lf/wave/`, `.lf/config.yaml`, or `main`. A first complete Initiative read binds its sole In Progress Linear Project (counted across the whole Initiative before Team filtering) under the planning lock; ambiguous inventories stay unconfigured with explicit choices. Rental `prepare` uses public `lf wave plan bind` when needed.
- `lf wave plan bind --replace` compare-and-replaces against the observed selection and settles only a generated first Project’s own pending transition.
- Placement writes one ownership entry naming the target Machine only after remote repo/server readiness. Repeating an interrupted placement inspects the recorded owner’s Graph before admitting another rental.

## 2a. Built — periodic & final rental collection

Jack Heart: “we need some way to capture session history / usage data for analytics for sandboxes that we lose acces to”

- The server recovery pass collects over SSH while each rental runs (even without provider API credentials), tracking one change position per Session in `session_export_changes` across both state and history DBs and checking open transcripts by mtime. Clean give-back revisits the full rental window for late writes.
- Collected `Session`, `SessionEvent`, `History`, `Turn`, `Transcript`, and capture files live in `machine_records` and `<home>/machines/<origin>/runs`, tagged with origin Machine and Session. `lf session list`, `lf session history show <id> [--transcript]`, and `lf session usage --session <id>` already read them.

## 3. Built — portable profile retention, Machine-aware harnesses, and credential refresh

Jack Heart: “my config.yaml should more or less be copied over, crtainly all the skill routing”

- **First profile on a dev rental.** `stream` is `HostLocal`. Provisioning delivers the caller’s global portable profile over SSH stdin during `lf install server --profile-stdin`, so even the first direct SSH shell sees retained skill routes while keeping `stream: dev`. Per-declaration retention preserves independent edits on conflicting keys while retaining unrelated routes, and repairs older `applied: null` dev-stream conflicts on retry.
- **Machine-aware harness selection.** New primary Sessions, Graph workers, and `commit-message` helpers resolve their own skill on the execution Machine, choosing the first installed and logged-in configured model (Claude/Codex native login status or forwarded native token), keeping `-a` exact and established threads intact. A primary prepared before a login arrives keeps its Session ID and binds its provider on first input. Task-primary recovery waits quiescently on missing executable/login without spending start counts.
- **Credential refresh & scratch Markdown.** `lf machine forward <label> [--task <id> | --session <id>]` verifies the registered Machine on the SSH connection, replaces configured `forward.files` atomically via private temp files, and optionally restarts only the targeted Session. Recursive `scratch/` loading skips AppleDouble `._*` and unreadable hidden Markdown while preserving valid hidden notes and naming unreadable ordinary `.md` files.

## 4. Next pass — concept-review alignment (unify analytics, Session readiness, and rental settlement)

Post-build concept review (`scratch/concept-review.html`) and live Modal QA (`scratch/qa.html`) found three conceptual splits in `HEAD` (`afdf8d0b45`). Implement in this order so the highest-leverage analytics gap proves out first:

### 4.1 Unify collected rental Sessions into `SessionHistory`, `lf session usage`, and `lf session history search`

- **Experience:** Running Tasks on Modal and losing a sandbox never leaves a blank hole in `lf session usage --task <ID>`, `lf session usage --wave <W>`, `lf session usage --days <N>`, `lf session usage --weekly`, `lf session history search <text>`, or Desktop’s Telemetry / Task History views. Collected rental records appear beside local work, labeled with their origin Machine (`from modal-1`). `lf session usage --session <id>` remains available as a single-Session drill-down reading the exact same turns.
- **Storage & DTO (`loopflow-storage`, `loopflow-execution`, `swift/Loopflow`, `tests/fixtures/dto`):**
  - Add an explicit nullable origin field on `SessionHistory`:
    `#[serde(deserialize_with = "Option::deserialize")] pub origin: Option<SessionOrigin>` with `SessionOrigin { machine_id: MachineId, label: Option<String> }` (`None` for local records, `Some(...)` for copies from `machine_records`). Mirror it with explicit `decode(SessionOrigin?.self, forKey:)` in `swift/Loopflow/Services/RegistryQuery.swift` and update `tests/fixtures/dto/session_history_summary.json`, `tests/fixtures/dto/wave_detail.json`, `tests/fixtures/dto/server_schema.json`, and Swift/Rust DTO fixture tests.
  - Project copied rental Sessions from `machine_records` into `conversation_history` / `recent_conversation_history` (joining `task_id` / `wave_id` against local `tasks`, `waves`, and `projects` for `--task`, `--wave`, `--project`, and `--days` filters; reducing `RecordKind::Turn` usage even when a capture never settled before abrupt loss; resolving machine labels from `machines`). Keep `origin: Some(...)` distinct from local rows so moved Sessions preserve turns recorded on both Machines without double-counting, and read capture files from `copies_home(&home, &origin.machine_id)` for `--context` when present.
  - Keep queries indexed at 10,000 Tasks / 100,000 copied turns (verify with `EXPLAIN QUERY PLAN`; add a targeted index on `machine_records` if needed for time/task selection).
- **CLI (`usage.rs`, `history_transcript.rs`):**
  - Render `origin` clearly in `lf session usage` table rows (e.g., `from modal-1` when `origin` is `Some`).
  - Extend `lf session history search` to search `RecordKind::Transcript` rows in `machine_records` under the same `since`, `WorkFilter`, and case-insensitive prefilter, tagging hits with `origin` / `from modal-1`.
  - Keep `lf session usage --session <id>` consistent with the same origin label and turn totals.
- **Tests:** Extend `a_lost_rentals_transcript_and_usage_survive_on_its_collecting_machine` (and focused storage/CLI tests) to verify `lf session usage --days 1`, `lf session usage --task <ID>`, `lf session usage --wave <W>`, and `lf session history search <marker>` all return the lost Modal sandbox’s turns and transcript, plus Swift/Rust DTO round-trip tests and `EXPLAIN QUERY PLAN` check.

### 4.2 Unify Session startup readiness and `start_failure` across recovery, steer wakeup, and `lf machine forward`

- **Experience:** Whenever the Machine server attempts to start a stopped Session — Task-primary recovery, `lf steer` waking a stopped interactive Session, or `lf machine forward --task/--session` restarting a provider — a missing executable or login is detected before spawning a doomed PTY and recorded in `agent_sessions.start_failure`, visible in `lf session list` (`could not start: ...`).
- **Execution & server (`ops/recovery.rs`, `ops/human_session.rs`, `server/recovery.rs`, `server/wake.rs`):**
  - Generalize `ops::recovery::launch_failure` so it checks readiness for any Session (`&LfSession` / `(session_id, provider)`), not only `TaskConversation`.
  - In `server/wake.rs::start_stopped`: check `launch_failure` before `hold()`. If unavailable, record `start_failure` (`WAITING_FOR_HARNESS...`) and skip spawning the PTY; if `hold()` fails, record the error in `start_failure`; if `hold()` succeeds, clear `start_failure`.
  - In `ops/human_session.rs::restart_provider`: after files arrive, check `launch_failure` before stopping the existing provider or reporting `"Resuming ..."`. If the Session’s harness is still unavailable, record `start_failure` and return an actionable error (leaving unrelated state intact); when `SessionContinue` succeeds, clear `start_failure`.
  - In `ops/human_session.rs::session_ending`: display `start_failure` for any Session that recorded one (remove `.filter(|_| conversation)` so stopped repository/interactive Sessions show `could not start: ...` instead of silent `unknown`).
- **Tests:** Verify steering a stopped repository Session on a Machine without a usable login records `start_failure` and surfaces `could not start: ...` in `lf session list`, clears it once login arrives and a subsequent steer wakes it, and verify `lf machine forward <label> --session <id>` refuses cleanly when the refreshed login is still unusable for that Session’s harness.

### 4.3 Unify ended-rental settlement (`lf machine end`), live Machine listing (`lf machine list`), and Modal `OnDemand` JSON

- **Experience:** Running `lf machine end modal-1` on an already-terminated sandbox settles it cleanly without suggesting `--force` to discard work on a dead box. `lf machine list` shows live Machines rather than accumulating dead ephemeral Modal routes, while `lf ondemand list` holds rental history and `from modal-1` attribution stays intact.
- **Commands (`commands/rental.rs`, `rental/recovery.rs`, `commands/machine.rs`):**
  - Expose `rental::recovery::refresh` (or a shared `end_explicit` helper) and call `refresh(sqlite, &api, &rental)` at the start of `commands/rental.rs::end`. If the provider reports the sandbox already gone, settle it via `refresh` (`ended`, update ceiling estimate, return held Tasks) and report that `<label>` had already ended at `<provider>` with its ceiling estimate — no bogus `--force` refusal. Only a still-live sandbox that fails SSH asks for `--force`.
  - In `commands/machine.rs::listings` (and `MachineCommand::Status` when `label.is_none()`): omit Machines whose `rental_for_machine` is `!state.is_live()`, keeping the SQLite `machines` row intact for `machine_records` FK and `from <label>` labels, and still answering explicit `lf machine list <label>` and `lf machine status <label>`.
  - In `commands/rental.rs` (`OndemandCommand::Add`): store `modal_sizing: (provider == Provider::Modal).then_some(if *reserved { ModalSizing::Reserved } else { ModalSizing::Burst })` so `lf ondemand list --json` reports `"modal_sizing": "burst"` on the `OnDemand` declaration itself (while `rental_hardware()` continues defaulting legacy `None` declarations to `Burst`).
- **Tests:** Verify `lf machine end <label>` settles an externally terminated sandbox without `--force`, verify ended rentals disappear from default `lf machine list` while remaining resolvable in `lf ondemand list`, `lf machine status <label>`, and `from <label>` history/usage output, and verify `lf ondemand list --json` includes `"modal_sizing": "burst"` on a newly added Modal on-demand.

## Gate and demo

Use a disposable Linear-backed Task on a fresh Linux Machine. Quote, rent, advance its Graph, inspect the retained skill routes from a direct SSH shell, refresh a login, then give it back with ordinary `lf machine end <label>`. Check one ownership entry, the recorded ceiling estimate, an unchanged main checkout, and clean `lf machine list`. On a second rental, collect a known transcript and usage sample, cut access at the provider, settle with ordinary `lf machine end <label>` (no `--force`), and read the surviving records through `lf session usage --days 1`, `lf session usage --task`, and `lf session history search`.

LOO-1083 owns the server-routed CLI (`lf open --machine`), Desktop, and Mobile workspace. LOO-1084 owns cross-Machine steering and status.

## Docs

Pages touched (keep in sync after §4):

- `docs/ops.md` (Modal burst vs. `--reserved` sizing, ceiling cost estimates, `lf machine forward`, `lf machine end` settling gone sandboxes, container Linux without systemd/cron)
- `docs/subscriptions.md` (Machine-aware harness fallback, quiescent recovery & `start_failure` across recovery/steer/forward, `lf machine forward <label> [--task | --session]`)
- `docs/telemetry.md` (work-scoped `lf session usage` and `lf session history search` including collected rental Sessions with origin Machine, alongside `lf session usage --session <id>`)
- `docs/work.md` (read-only cold Wave acquisition on fresh clones, idempotent `lf wave plan connect`, and `lf wave plan bind --replace`)

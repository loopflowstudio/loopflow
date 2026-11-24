# LOO-1081 · Run a Task on Modal

**Done when** a Linear-backed Task runs its Graph on a fresh Modal Machine with the caller’s portable settings and a usable harness. An idle sandbox pays the small resource floor while quotes and caps cover peak compute. Giving it back needs neither a manual Git commit nor a database edit. Session history, transcripts and recorded usage remain available for analytics after abrupt loss.

## Model

1. **Adopting an authored Wave is separate from authoring a new one.** Cold acquisition on a fresh clone registers existing `.lf/wave/` files without writing or committing to `main` and without inventing a local `tracker_id: null` Project. It binds the sole In Progress Linear Project from a complete Initiative inventory; explicit `lf wave plan bind` handles ambiguous or misselected Waves.
2. **Placing an unowned Task on a Machine is one atomic ownership entry.** Never claim locally first and move second. Write the target owner only after the remote checkout and server are ready.
3. **Modal sizing separates burst ceiling from idle floor.** `--vcpus` and `--memory-gb` are burst ceilings with Modal’s minimum floor (`0.125` physical core, `128 MiB`) unless `--reserved` pins floor = ceiling. Admission, caps, and timeouts price the ceiling; uptime-derived cost is explicitly labeled as a ceiling estimate.
4. **Container Linux is an ordinary Machine.** Missing systemd or cron never blocks `lf server connect` or `lf install`. Rentals receive the caller’s matching Linux `lf` build before remote GitHub login exists.
5. **Execution adapts to the Machine’s installed harnesses; recovery never crash-loops on missing ones.** Personal preferences fall back to usable installed native harnesses on the execution Machine (`-a` stays exact; established threads keep their identity). A missing executable or login leaves recovery quiescent until inputs change.

## 1. First slice — admit one usable Modal Machine

Run the normal rent path through quote, create, non-login SSH, installation, and a ready Machine server before handing over a Task. This validates the two foundations every later slice depends on: Modal honors burst limits and the remote answers our RPCs.

- **Built — Modal burst sizing and ceiling estimates.** Admission records the sizing mode in the rental’s hardware. Old declarations acquire burst limits on their next admission. A rental with no recorded limits retains its historical hardware and cost. New Modal rentals expose `ceiling_estimate_usd` in CLI JSON. There is no Swift rental DTO or shared rental fixture. The storage test exercises the public JSON serialization after reading the admitted rental back from SQLite.
  - For 8 vCPU / 16 GiB, the pinned Modal 1.6.2 SDK encoded the bridge’s CPU and memory requests/limits as `125/4000` millicores and `128/16384` MiB in burst mode, and `4000/4000` and `16384/16384` with `--reserved`. The SDK leaves the maxima unset for scalars, so both modes send tuples. Both use the checked-in ceiling of $0.95184/h for admission. Modal’s timeout takes the remaining whole seconds at create.
  - Jack Heart’s billing steer: “Billing data point: actual Modal dashboard usage was $1.65 vs ~$1.78 from lf fixed uptime math ($0.95184/h across modal-1..6), confirming lf currently sends --vcpus/--memory-gb as a full request floor rather than burst-only limits.” The dashboard figure is an aggregate observation, not an idle-rate measurement.
  - `uv run pytest -q python/tests/test_rental_bridge.py -k modal` and `cargo test -p loopflow-storage --lib modal_` each passed three tests. The Task CLI fixture `task_run_open_successful_quote_launches_and_opens_nothing` passed.
  - Private Machine smoke: `lf task run DEMO-1 --rent <name> --quote` printed $0.95184/h and $3.80736 for four hours in each mode. `lf ondemand rent <name> --max-usd 0.47592` reached the SDK stand-in with the expected tuples and 1798/1799 seconds remaining. No provider call was made.
- **Built — caller-resolved Linux packages.** Release callers fetch their tagged Linux archive and verify its SHA-256. Main packages carry the GitHub run ID that produced them. A clean source build can also look up packages by commit. The caller selects the sandbox's architecture, uploads the CLI, and checks the promoted revision and serving executable before recording the Machine. `LF_RENT_LINUX_BIN` still selects an unpublished executable with its private Machine home. The shared image includes Codex and Claude Code and puts `/home/lf/.local/bin` on the non-login SSH path.
- **Built — container installation and server readiness.** With no systemd user manager, the selected CLI starts a detached server under the install lock. A serving superseded binary can retire without ending its holders. Shells reach the same socket after the SSH connection ends. A missing crontab leaves automatic updates unscheduled, and an explicit scheduling request reports the error. Promotion reads process rows on stores predating the move to owner records.
  - An ARM64 Linux container ran `lf ondemand rent modal` over real key-only, non-login SSH. The provider and GitHub Actions downloads were fixtures. The remote started at the published v0.15.1, kept the same Machine ID after promotion, and answered `lf server status` with the selected build. A second promotion changed the serving hash and instance. `lf install --dev` succeeded without cron and `lf install schedule` reported it missing.
  - The first rent reached promotion but failed readiness on systemd 257, which says “$DBUS_SESSION_BUS_ADDRESS and $XDG_RUNTIME_DIR not defined”. That failure returned the confirmed sandbox to the provider fixture. The fallback now recognizes the absent bus.
  - A second SSH endpoint was a fresh Ubuntu 24.04 container without `systemctl` or `crontab`. Its packaged server's SHA-256 matched the caller's Linux archive. Hiding the archive produced a revision-specific override diagnostic, one create and one delete, and an ended rental with no Machine. `lf machine end fresh-1` returned the working rental. The explicit `LF_RENT_LINUX_BIN` upload also reached a ready server in a fresh `.lf-dev` and returned with ordinary `lf machine end`.
  - The private override first failed with ENOENT because socket discovery canonicalized a Machine home that did not yet exist. Server startup now creates that directory. An explicitly masked unit and a user-bus permission denial both remained errors.
  - The focused `container_connects_share_one_server_and_restart_it_after_exit` Linux fixture passed over the real server protocol. The v0.15.1 inventory fixture found its live Graph process before and after migration and surfaced a missing index on the migrated store. The new schema lookup used the covering primary-key index, averaging 1.1 µs across 1,000 reads on a copy with 10,000 historical processes.
  - The Modal bridge, pinned artifact and automatic scheduling fixtures passed. Gate still owns a live provider run and the full two-Machine scenario.

## 2. Adopt the existing Wave and hand off atomically

- **Built.** Cold registration reads the Wave identity from authored files. A first complete Initiative read binds its sole In Progress Project under the planning lock. An ambiguous inventory leaves the choice open. Rental preparation reads that choice on the destination and binds the source's tracker Project there when needed. Reconnecting or re-adding the same Wave leaves its files alone.
- `lf wave plan bind --replace` compares the observed selection in the write transaction. Replacing a generated first Project settles only its own pending transition and keeps both Project rows.
- Placement records the target Machine and its outgoing ownership marker together. Repeating an interrupted placement checks the recorded owner's Graph before admission or dispatch.
- Two isolated Machines exercised a broken launch connection. The ownership log held one entry naming the destination, and retry observed the existing Graph. A private CLI Machine accepted a legacy GOAL.md Wave by durable ID on its first `lf task list`; the clone kept its original HEAD and clean index. The Linear fixture exercised sole and ambiguous active Projects, reconnect, and a refused terminal replacement. Gate covers live Modal with Linear and the full prepared-rental handoff.

- SQLite query plans at 1,000 Waves and 10,000 Projects used the Wave membership index, then keyed Project lookups. In warm reads the candidate query averaged about 47 µs and the inventory marker lookup about 2 µs.
- `cargo nextest run -p loopflow-execution -p loopflow-storage --lib` passed the four named adoption, retirement and transition cases. `cargo test -p loopflow-execution --lib cold_clone_adopts_its_wave_and_only_the_unambiguous_project` also passed with a Task in the imported Project. `cargo clippy --all-targets -- -D warnings` passed. The two-Machine CLI test `a_placed_task_retries_on_its_owner_after_the_launch_connection_breaks` passed. The Swift decoder read both shared Wave receipts and rejected an absent Project key. Gate runs the broader planning and ownership suites.

## 2a. Retain analytics after losing a rental

Jack Heart: “we need some way to capture session history / usage data for analytics for sandboxes that we lose acces to”

- **Next slice.** Collect while the sandbox is running, then finish the same collection on ordinary give-back. The current recovery path calls `copy_up` only on give-back. Its export copies Sessions, events and capture files. The local usage and transcript readers still need the originating turn and usage records.
- Reuse the origin Machine and Session identities in `machine_records`. Capture observed turns, transcripts and token/cost facts on a durable Machine outside the sandbox. Resume incrementally after disconnection and deduplicate across live collection and the final copy. Surface the last successful collection when a sandbox disappears mid-turn.
- Exercise two Machines with a live Session. Observe a turn and known usage on the collecting Machine, interrupt transport, then remove access to the sandbox without running `lf machine end`. Verify local history, transcript and usage totals after restart, including their origin. Repeated collection and clean teardown must count the same turn once.

## 3. Run on available harnesses and refresh forwarded logins

Jack Heart: “my config.yaml should more or less be copied over, crtainly all the skill routing”

- During rental provisioning, install the caller’s portable personal configuration on disk through the existing profile projection and retention writer. Include all skill routes, agents, default agent and `yolo`. Keep host-local connections, on-demands and forwarding rules on the source Machine. Verify the first direct SSH shell can read those routes before a forwarded launch. Then update one route and verify background Graph steps and primary Sessions read the new value without a retention conflict.
- **Retain the first forwarded profile on a dev rental.** On modal-2 through modal-6, the install wrote `stream: dev` before profile delivery. Retention recorded a conflict with `applied: null`; `lf config show` reported `agent: Not selected`. In `config/remote.rs` and the setting catalog, compare the incoming portable settings with the corresponding local keys. Keep the bootstrap stream and independent edits. Exercise first delivery with just the stream present, with matching portable values, and with a conflicting personal edit.
- **Machine-aware harness resolution and quiescent recovery (`ops/recovery.rs`, `server/recovery.rs`, session/graph launch):**
  - Resolve the named skill on the execution Machine before stamping a primary Session’s provider or starting a Graph worker, using existing launcher availability and native login checks.
  - Exercise the retained `skills.<name>.agents` and `default_agent` ordering for `commit-message`, `loop-or-next` and `task-session`. With agy absent, Claude installed without a login, and Codex signed in, select the skill’s configured Codex model (e.g. `gpt-6.2-sol`). Repeat with a usable Claude login. An explicit parent launch override must not become the implicit model for independently launched helpers.
  - Field count from the five rentals: 99.5% of turns went to `gpt-6.2-astra`, including 246 commit-message Sessions and 399 task-session wakeups (72.7M cached input).
  - A missing configured harness yields to the first usable configured fallback, then a usable installed native harness. Record the selected provider/model before recovery reads the Session. Explicit `-a <agent>` stays exact; an established thread keeps its identity.
  - If no candidate works (or an established thread’s harness is unavailable on this Machine), report which executables or logins are missing and treat missing executable/login as non-retryable in `ops/recovery.rs` — leaving recovery quiescent until inputs change instead of burning five backoff retries. Keep quota recovery and provider thread moves intact.
- **Explicit credential re-forwarding (`machine/forward.rs`, `commands/machine.rs`):**
  - Ordinary remote launches keep destination-wins behavior in `machine/forward.rs::deliver`; per-launch `forward.env` stays ephemeral.
  - Add `lf machine forward <label> [--task <id> | --session <id>]` to refresh configured `forward.files` on a live Machine. Read through the existing `forward` configuration and replace each complete file atomically via a unique private temporary file (`umask 077`) over SSH stdin.
  - With `--session <id>` or `--task <id>` (resolving that Task’s primary Session on `<label>`), restart only that Session’s provider on its owner after delivery succeeds, retaining the Session and continuation. Failure before delivery completes starts nothing. Test with synthetic rotated values, a blocked login prompt, and an unrelated Session that keeps running.
- **Skip AppleDouble and binary hidden Markdown (`prompt/mod.rs::gather_md_files_from`):**
  - Skip macOS AppleDouble (`._*`) paths and unreadable-as-UTF-8 hidden `.md` dotfiles during recursive `scratch/` collection. Keep valid hidden notes, nested Markdown ordering, symlink refusal, and errors for ordinary unreadable `.md` files.

## Gate and demo

Use a disposable Linear-backed Task on a fresh Linux Machine. Quote, rent, advance its Graph, inspect the retained skill routes from a direct SSH shell, refresh a login, then give it back with ordinary `lf machine end <label>`. Check one ownership entry, the recorded ceiling estimate and an unchanged main checkout. On a second rental, collect a known transcript and usage sample, cut access, and read them on the surviving Machine.

LOO-1083 owns the server-routed CLI, Desktop and Mobile workspace. LOO-1084 owns cross-Machine steering and status.

## Docs

The docs written ahead of implementation (eb22482548) still describe `lf machine forward` and harness fallback. The Modal sizing, Linux package selection, container startup, Wave adoption and Project binding passages now match the code. Before gate, cut any passage whose slice did not ship. The first command still fails `documented_commands` until it ships.

Pages touched:

- `docs/ops.md` (Modal burst vs. `--reserved` sizing, ceiling cost estimates, `lf machine forward`, container Linux without systemd/cron)
- `docs/subscriptions.md` (Machine-aware harness fallback, quiescent recovery on missing executable/login, `lf machine forward <label> [--task | --session]`)
- `docs/work.md` (read-only cold Wave acquisition on fresh clones, idempotent `lf wave plan connect`, and `lf wave plan bind --replace`)

Open product question from writing the docs:

- When `lf machine forward <label> --task <id>` or `--session <id>` restarts a Session's provider after refreshing `forward.files`, should that restart also pass current `forward.env` values (for example, a rotated `CLAUDE_CODE_OAUTH_TOKEN`) ephemerally into the restarted provider launch, or should `lf machine forward` refresh `forward.files` only?

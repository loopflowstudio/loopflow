# LOO-1081 · Run and open a Task on Modal

**Done when** a Task Session can send a Linear-backed Task to a fresh Modal Machine, advance its Graph, and open its work from the laptop or paired phone. Files show the remote working copy, Changes shows its uncommitted diff, a shell starts in that checkout, and the conversation resumes the same remote Session. An idle sandbox pays the small resource floor while quotes and caps still cover peak compute. Giving the Machine back needs neither a manual Git commit nor a database edit.

The field case was `manabot`, Wave `coverage`, Project `rules`, Tasks MTG-418 through MTG-422. For MTG-421 the Desktop Session pane reported “The conversation belongs to a different Machine or checkout; nothing was opened.” Its shell opened on the Mac. Use disposable counterparts for the demo.

## Model

Six principles tie this work together across CLI, server, Desktop, and Mobile:

1. **One server-side router owns cross-Machine Task workspaces.** A Task’s owning Machine (from the ownership log) serves its Files, Changes, Diff, Save, Shell, and Session. The local/paired Loopflow server routes all Task-scoped reads, writes, and terminal tickets to that owner over a pooled `Connection::open_on` connection — serving Desktop and Mobile through the exact same protocol instead of maintaining a separate client-side SSH-subprocess path in Desktop.
2. **Adopting an authored Wave is separate from authoring a new one.** Cold acquisition on a fresh clone registers existing `.lf/wave/` files without writing or committing to `main` and without inventing a local `tracker_id: null` Project. It binds the sole In Progress Linear Project from a complete Initiative inventory; explicit `lf wave plan bind` handles ambiguous or misselected Waves.
3. **Placing an unowned Task on a Machine is one atomic ownership entry.** Never claim locally first and move second. Write the target owner only after the remote checkout and server are ready.
4. **Modal sizing separates burst ceiling from idle floor.** `--vcpus` and `--memory-gb` are burst ceilings with Modal’s minimum floor (`0.125` physical core, `128 MiB`) unless `--reserved` pins floor = ceiling. Admission, caps, and timeouts price the ceiling; uptime-derived cost is explicitly labeled as a ceiling estimate.
5. **Container Linux is an ordinary Machine.** Missing systemd or cron never blocks `lf server connect` or `lf install`. Rentals receive the caller’s matching Linux `lf` build before remote GitHub login exists.
6. **Execution adapts to the Machine’s installed harnesses; recovery never crash-loops on missing ones.** Personal preferences fall back to usable installed native harnesses on the execution Machine (`-a` stays exact; established threads keep their identity). A missing executable or login leaves recovery quiescent until inputs change.

## 1. First slice — admit one usable Modal Machine

Run the normal rent path through quote, create, non-login SSH, installation, and a ready Machine server before handing over a Task. This validates the two foundations every later slice depends on: Modal honors burst limits and the remote answers our RPCs.

- **Built — Modal burst sizing and ceiling estimates.** Admission records the sizing mode in the rental’s hardware. Old declarations acquire burst limits on their next admission. A rental with no recorded limits retains its historical hardware and cost. New Modal rentals expose `ceiling_estimate_usd` in CLI JSON. There is no Swift rental DTO or shared rental fixture. The storage test exercises the public JSON serialization after reading the admitted rental back from SQLite.
  - For 8 vCPU / 16 GiB, the pinned Modal 1.6.2 SDK encoded the bridge’s CPU and memory requests/limits as `125/4000` millicores and `128/16384` MiB in burst mode, and `4000/4000` and `16384/16384` with `--reserved`. The SDK leaves the maxima unset for scalars, so both modes send tuples. Both use the checked-in ceiling of $0.95184/h for admission. Modal’s timeout takes the remaining whole seconds at create.
  - Jack Heart’s billing steer: “Billing data point: actual Modal dashboard usage was $1.65 vs ~$1.78 from lf fixed uptime math ($0.95184/h across modal-1..6), confirming lf currently sends --vcpus/--memory-gb as a full request floor rather than burst-only limits.” The dashboard figure is an aggregate observation, not an idle-rate measurement.
  - `uv run pytest -q python/tests/test_rental_bridge.py -k modal` and `cargo test -p loopflow-storage --lib modal_` each passed three tests. The Task CLI fixture `task_run_open_successful_quote_launches_and_opens_nothing` passed.
  - Private Machine smoke: `lf task run DEMO-1 --rent <name> --quote` printed $0.95184/h and $3.80736 for four hours in each mode. `lf ondemand rent <name> --max-usd 0.47592` reached the SDK stand-in with the expected tuples and 1798/1799 seconds remaining. No provider call was made.
  - **Next:** finish the image tools and caller-resolved Linux installation below, then check container server readiness. The paid sandbox, Task handoff and remote views remain for gate and demo.
- **Image tools and caller-resolved Linux `lf` (`sandbox.py`, `commands/rental.rs::attach`, `commands/install/published.rs`):**
  - Install `@openai/codex` beside `@anthropic-ai/claude-code` in the shared image `TOOLS`, and include `/home/lf/.local/bin` in `/etc/environment` `PATH` so non-login `ssh ... lf ...` finds `lf`.
  - In `commands/rental.rs::attach`, resolve the Linux package **on the caller** using the caller’s stream/revision and remote architecture, reusing `commands/install/published.rs` release verification and authenticated CI artifact retrieval. Match the caller’s release tag or packaged dev revision, transfer the candidate over SSH, and run its normal promotion on the rental (so dev artifact access works before remote GitHub login). Keep `LF_RENT_LINUX_BIN` for unpublished local builds. If the matching artifact is gone, report the revision and override before any Task claim. Verify the selected build and server readiness before handoff. Keep one provider create per admission, lookup by name on an unknown result, and cleanup of a confirmed unused sandbox.
- **Container-safe server and install (`installation/server.rs::ensure`, `commands/install/published.rs`, `commands/binaries.rs`):**
  - Under the existing install lock in `installation/server.rs::ensure`, use the Linux systemd user service when available. When the manager or user bus is absent, detach the selected `lf server run` with its Machine home, closed stdin, and `logs/server.log`. Concurrent connects share its socket and readiness/hash check; reconnect after exit starts one replacement. Still honor explicitly disabled systemd units and report genuine service failures.
  - In `commands/install/published.rs`, when `crontab` is absent after successful promotion, report that automatic updates are unscheduled and return success. Explicit `lf install schedule` still reports the failure.
  - Exercise promotion from the actual `v0.15.1` schema, which lacks `agent_sessions_attached`. Current `binaries::report_before_promote` calls `recorded_lfs()` on the unmigrated read-only store where the query forces `INDEXED BY agent_sessions_attached`. Inspect live `lf` processes using a query compatible with pre-migration schemas (retaining unknown-inventory and backup protection on genuine read failures) and verify post-migration records and indexes.

## 2. Adopt the existing Wave and hand off atomically

`checkout_script` binds the repository ID. Today `ops/linear.rs::wave_for_initiative` searches only registered SQLite Waves, and `connect_async` calls `wave_lifecycle::add`, which runs `ensure_first_project` (inventing a local `tracker_id: null` Project) and commits normalized YAML to `main`. Separate adopting an authored Wave from creating a new one, and replace two-step ownership handoff with one atomic entry.

- **Read-only cold Wave acquisition (`work/wave/bootstrap.rs`, `ops/linear.rs`, `ops/wave_lifecycle.rs`):**
  - Separate read-only registration of existing `.lf/wave/` files from file seeding and new-Wave authoring (`wave_lifecycle::add`).
  - Run cold Wave acquisition by durable ID across `task list`, `task checkout`, `wave_for_initiative`, `create`-existing, and `connect`, reusing the discovery and retirement rules in `work/wave/bootstrap.rs`. Never create files, never invent a local `tracker_id: null` Project, and never commit to `main` during cold acquisition.
  - Import tracker inventory under the Wave planning lock and bind the **sole In Progress Linear Project** from a complete Initiative inventory. Missing, ambiguous (0 or >1 In Progress Projects), or offline inventory leaves the Wave unconfigured with a clear reason and the available Projects. Repeated acquisition respects existing choices and unfinished transitions.
  - During rental `prepare`, if the Task’s Wave on the source Machine has a bound tracker Project ID and cold acquisition left the Wave unconfigured (e.g., multiple In Progress Projects), bind that exact Project via the existing public `lf wave plan bind` command — no rental-only carried-selection state.
- **Idempotent reconnect and explicit rebind (`ops/linear.rs`, `ops/wave_lifecycle.rs`):**
  - Reconnecting an already-bound Team and Initiative returns before normalizing legacy YAML, leaving `.lf/config.yaml`, `.lf/wave/`, Git index, and `main` HEAD untouched.
  - A genuinely new Wave still creates its authored files and first Project; an intentional first connection still commits its authored binding. Keep the CLI/Swift `WaveAdded` receipt accurate when joining an existing Wave.
  - Support repairing a previously misselected Wave via an explicit compare-and-replace on `lf wave plan bind` (e.g. `--replace`), retaining the old Project, work, and history and resolving any generated pending transition by its matching identity. Never treat a name or `tracker_id: null` alone as disposable.
- **Single atomic ownership handoff (`commands/rental.rs::hand_off`, `ops/task_ownership.rs`, `store/ownership.rs`):**
  - Only after remote checkout readiness, hand off an unowned Task with **one** ownership entry naming the target Machine (and use normal `Move` when the Task is already owned locally). Replace the unowned path’s separate local `Claim` then `Move`.
  - Retain existing writer and person checks. Record the ownership entry and its outgoing marker together so a late or redelivered predecessor cannot reclaim the Task on the laptop. Retry after an uncertain remote start inspects the recorded owner and Graph before renting or launching again, preserving genuine later moves and work on an unreachable owner.

## 3. Run on available harnesses and refresh forwarded logins

- **Machine-aware harness resolution and quiescent recovery (`ops/recovery.rs`, `server/recovery.rs`, session/graph launch):**
  - Resolve the named skill on the execution Machine before stamping a primary Session’s provider or starting a Graph worker, using existing launcher availability and native login checks.
  - A missing configured harness yields to the first usable configured fallback, then a usable installed native harness. Record the selected provider/model before recovery reads the Session. Explicit `-a <agent>` stays exact; an established thread keeps its identity.
  - If no candidate works (or an established thread’s harness is unavailable on this Machine), report which executables or logins are missing and treat missing executable/login as non-retryable in `ops/recovery.rs` — leaving recovery quiescent until inputs change instead of burning five backoff retries. Keep quota recovery and provider thread moves intact.
- **Explicit credential re-forwarding (`machine/forward.rs`, `commands/machine.rs`):**
  - Ordinary remote launches keep destination-wins behavior in `machine/forward.rs::deliver`; per-launch `forward.env` stays ephemeral.
  - Add `lf machine forward <label> [--task <id> | --session <id>]` to refresh configured `forward.files` on a live Machine. Read through the existing `forward` configuration and replace each complete file atomically via a unique private temporary file (`umask 077`) over SSH stdin.
  - With `--session <id>` or `--task <id>` (resolving that Task’s primary Session on `<label>`), restart only that Session’s provider on its owner after delivery succeeds, retaining the Session and continuation. Failure before delivery completes starts nothing. Test with synthetic rotated values, a blocked login prompt, and an unrelated Session that keeps running.
- **Skip AppleDouble and binary hidden Markdown (`prompt/mod.rs::gather_md_files_from`):**
  - Skip macOS AppleDouble (`._*`) paths and unreadable-as-UTF-8 hidden `.md` dotfiles during recursive `scratch/` collection. Keep valid hidden notes, nested Markdown ordering, symlink refusal, and errors for ordinary unreadable `.md` files.

## 4. One server-routed Task workspace across CLI, Desktop, and Mobile

Ownership selects the execution Machine. The local/paired Loopflow server is the single cross-Machine router for Task workspaces, serving CLI, Desktop, and Mobile through one protocol and deleting Desktop’s duplicate client-side SSH-subprocess routing stack.

- **Server-side Task routing and terminal ticket bridging (`commands/read.rs`, `commands/write.rs`, `server/service.rs`, `machine/client.rs`, `commands/waves.rs`):**
  - Maintain a pooled/cached `Connection::open_on` per peer Machine on the server (reconnecting on error/EOF, with idle eviction) so browsing remote files and diffs does not pay a cold SSH handshake per request.
  - Route all Task-scoped `Read` operations (`task.status`, `task.get`, `task.detail`, `task.changes`, `task.diff`, `task.file`, `task.files`) in `Read::read()` to the Task’s owning Machine when `Origin::Local` and another Machine owns it, matching how `task.status` already forwards via `Origin::Machine`.
  - Add `task.save` to `Write::remote()` beside `task.run`, `task.auto.*`, `task.claim`, `task.unassign`, and `session.resume`.
  - Route remote terminal operations (`session.attach`, `session.continue`, `session.start` / Task shell) in `server/service.rs`: when the target Session or Task shell lives on another Machine, open/reuse `Connection::open_on` to that owner, acquire the owner’s terminal/ticket, and issue a single-use, admission-bound ticket on the local/paired server that bridges stream and bulk bytes to the owner’s terminal. Reuse `session.start`, attachment, resize/input, and reconnect semantics from `server/README.md`.
  - Populate `RoadmapTask` / `TaskReferenceSnapshot` from actual Task ownership (`task_execution_route`) rather than local `task_checkouts` alone — never exposing a stale local worktree as the active workspace when another Machine owns the Task.
  - Support optional expected-owner checks on Task-scoped reads/writes so a Task move returns an immediate `conflict` from local SQLite before forwarding. Distinguish an unstarted Task (“not started”, no checkout yet) from an unreachable owner (`unavailable`, naming the Machine — never an empty diff or local shell). Maintain protocol schema (`tests/fixtures/dto/server_schema.json`), fixtures, and shared Swift decoding.
- **Desktop (`swift/LoopflowMac/WorkModel.swift`, `Views/SessionsView.swift`, `swift/Loopflow/Services/RegistryQuery.swift`):**
  - Drive remote Task Files, Changes, Diff, Save, Shell, and Session through the local Loopflow server’s routed operations — **deleting** the client-side `RegistryQuery.onMachine` CLI-over-SSH fanout and double-SSH `pinTaskLocation` wrapper for Task views.
  - Resolve the Task’s owner from `RoadmapTask` / server readings before selecting retained panes, Sessions, or shells (including cold open, restoration, prefetch, and tab creation). Keep the placement check that caught the wrong record; invalidate stale panes, file watches, and attachments when ownership changes while retaining unsaved drafts at their original `(machine_id, checkout)`.
- **Mobile (`swift/LoopflowiOS/TaskWorkspaceView.swift`, `RemoteMobileClient.swift`):**
  - Expose Files (directory tree and file content, including unchanged files) and Shell alongside Changes and Session, calling the exact same paired-server operations and bridged terminal tickets. The phone still needs only one pairing.
  - Show “not started” when the owner has no checkout; show an explicit error when the owner is unreachable (never an empty diff or local shell).
- **CLI (`commands/open.rs`, `commands/open/conversation.rs`, `bin/lf.rs`):**
  - `lf open` always presents on the calling Machine (never executes remotely on a headless container).
  - `lf open --task <id> --tab` and `lf open --task <id> --session <sid> --tab` open a local tab attaching on the owning Machine, keeping `--task` / owner Machine and repository in `resume_argv`, exact-ID lookup, attachment receipt, and retry (fixing `open.rs::resume_argv` dropping `--task` when `--session` is set).
  - For a remote-only Session with no local Task, accept `--machine <label>` with `lf open --session <sid> --tab` (in either flag position: `lf open --machine <label> --session <sid> --tab` or `lf --machine <label> open --session <sid> --tab`), resolving and attaching on `<label>` while presenting here. An unavailable owner yields a retryable failure.

## Gate and demo

One two-Machine scenario runs through the public entry points. The fixture has a disposable Linear-backed repo, an active `rules` Project and a historical Project, a stale local checkout with different file bytes, and a fresh Linux owner without systemd or cron. The named checks are:

```sh
uv run pytest python/tests/test_rental_bridge.py -k modal
cargo test -p loopflow --test integration machine_commands::modal_task_workspace
scripts/test_desktop.sh --filter RemoteTaskWorkspaceTests
uv run python scripts/test_ios.py -only-testing:LoopflowiOSTests/RemoteTaskWorkspaceTests
```

- The bridge test inspects SDK create arguments for burst and `--reserved`.
- The CLI scenario quotes, rents, advances a real fixture Graph (with `scratch/plan.md` beside binary `scratch/._plan.md`), observes one atomic ownership entry despite delayed marker delivery, rotates a synthetic login via `lf machine forward`, and opens the same Session twice via `lf open`.
- Both Desktop and Mobile tests exercise all four viewers (Files, Changes, Shell, Session) through the server-routed protocol against distinguishable Machine responses: assert remote-only **unchanged** file contents, an **uncommitted** diff hunk, shell output from `hostname` and `pwd`, and an input acknowledgement from the exact remote Session. Disconnect, reopen, and move the Task while a pane is retained; verify each action reaches the current owner or reports why (preserving unsaved drafts on move).
- Run old-store (`v0.15.1`) promotion and concurrent `lf server connect` without systemd/cron on Linux.
- Finish with ordinary `lf machine end <label>` (no `--force`), verifying remote records returned, cost labeled as a ceiling estimate, and `main` still at the starting commit.

For the installed demo, run `lf task run <demo-task> --rent modal --quote`, then repeat without `--quote`. Open the Task with `lf open --task <demo-task> --tab`, Desktop, and the paired phone. Confirm all four viewers on the remote sandbox, then give it back with `lf machine end <label>`.

## Docs

Updated user docs in place:

- `docs/ops.md` (Modal burst vs. `--reserved` sizing, ceiling cost estimates, `--machine` exception for `lf open`, `lf machine forward`, container Linux without systemd/cron)
- `docs/subscriptions.md` (Machine-aware harness fallback, quiescent recovery on missing executable/login, `lf machine forward <label> [--task | --session]`)
- `docs/workspaces.md` (`lf open` always presenting locally, remote Tasks, and `--machine <label> --session <id>`)
- `docs/surfaces.md` (Desktop server-routed Files, Changes, Diff, Save, Shell, and Session for remote Tasks; unreachable and move handling)
- `docs/server.md` (Mobile single-pairing access to Files, Changes, Shell, and Session on remote Tasks)
- `docs/work.md` (read-only cold Wave acquisition on fresh clones, idempotent `lf wave plan connect`, and `lf wave plan bind --replace`)
- `docs/lf-reference.md` (`--machine` note for `lf open`)

Open product question from writing the docs:

- When `lf machine forward <label> --task <id>` or `--session <id>` restarts a Session's provider after refreshing `forward.files`, should that restart also pass current `forward.env` values (for example, a rotated `CLAUDE_CODE_OAUTH_TOKEN`) ephemerally into the restarted provider launch, or should `lf machine forward` refresh `forward.files` only?

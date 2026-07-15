# v0.11.1

v0.11.0 collapsed the runtime to waves, projects, and tasks. This release makes the surfaces on top of it honest. Wave Chat now reads as a conversation and streams at conversational speed instead of about a word per second. A Session records the binary and store it was born with, so a supervisor can no longer relaunch it against the wrong database or restart work you already abandoned. `lf status` finally reports the runs a wave produced and the work waiting on somebody — and says so plainly when it could not look. The release pipeline that wedged on v0.11.0 is repaired end to end.

## Wave Chat is a conversation again

Chat was rendering the execution log. Every tool call, shell command, file edit, and state transition got its own line in the CLI and its own card on the Mac, so a single long task flow buried what the wave actually *said* under twenty-odd pieces of backend evidence. The thread now carries speech, decisions, deliveries, and human-level failures — nothing else. The evidence still lives in the journal, where it belongs.

The same view was also slow, and the cause was worth measuring rather than guessing. `URLSession.AsyncBytes` yields one byte per `await`, and the chat connection is `@MainActor`, so the read loop paid a main-actor hop per byte — 0.14 MB/s. That is survivable only while frames are small, and they are not: the listener re-sends the whole open turn on every token, so a frame reaches ~106 KB and one turn puts ~68 MB on the wire. Reading in chunks pays one hop per network read instead of one per byte.

- **The thread renders speech** — the CLI emits `you › …` / `wave › …` prose, prints the wave's speech whole rather than ellipsized, and drops runtime turn ids. Child activity surfaces only when a human needs it: decision required or resolved, control uncertain, PR opened, completed, failed. Lifecycle churn like `state_changed` stays silent (#878).
- **Streaming is fast** — a single frame took ~734 ms to read byte-at-a-time and the connect replay took ~22 s; the chunked reader measures >200 MB/s. Parsing inside a chunk stays byte-level so the blank line delimiting an SSE frame still registers. Benchmarks on both sides (`cargo bench --bench wave_stream`, `swift/Benchmarks/wave_chat_render.swift`) and a test-held budget keep it there (#877).
- **Connecting to a long-lived wave is cheap** — the runtime replays the latest 12 turns instead of the entire history, so resuming doesn't dump the whole thread into your terminal (#878).
- **The Mac app follows** — a `WaveChatTranscript` projection turns wire turns into conversational messages, which let `MessageRow` shed its per-item card rendering and collapsed `WaveChatView` to a single conversation view (#878).

Note for scripts: `--json` on the `lf chat --follow` path is gone. The thread is a conversation; the raw frames were a second view nobody used.

## Sessions recover without relaunching the wrong thing

A Session used to re-derive its execution context from whichever process happened to be holding it. That meant a worktree's `target/debug/lf` could relaunch a Session against the live registry — whoever typed the command chose the child's binary and store. Abandonment had a matching hole: intent was recorded when a runner *consumed* the command, not when it was queued, and in that gap a supervisor would cheerfully restart work someone had already ended.

- **Execution context is pinned at birth** — `lf_bin`, `db_path`, and `lf_home` are recorded on the Session and reproduced on every relaunch. A Session created before this migration has no pinned context and none can be invented for it, so it refuses to launch and says why rather than guessing (#882).
- **Terminal intent is durable** — `abandon_requested_at` and `abandon_reason` are written the moment the Abandon command is queued. A supervisor reading the Session sees the abandonment, not a stale `Running` (#882).
- **tmux-launched sessions land on the right store** — the session shell command clears inherited `LF_RUN_ID`, `LF_PROCESS_ID`, `LF_HOME`, and `LF_DB_PATH`, and explicit env wins over inherited (#882).

## `lf status` delivers what it promised

`lf status` claimed to be a wave's audit surface but reported only the project/task hierarchy and loop state. The runs a wave had produced and the work sitting on somebody were both missing.

- **Runs and attention are on the wire** — both land in `WaveDetailSnapshot`, print in the human view, and are mirrored in Swift. A run is now one agent-backed skill call, the grain that owns context, tokens, cost, and outcome; status filters those runs to its Wave. Process-level lookups moved to `lf execs`, and `lf trace <exec-id>` owns process-tree diagnosis. An active run is never dropped by the window cap (#881).
- **"Found nothing" and "could not look" are different facts** — both fields ride a new `Evidence<T>` wire enum: `Ok { items, truncated }` or `Unavailable { reason }`. A status surface that renders a failed ledger read as an empty list is lying to whoever is on call, and `truncated` keeps a full page from reading as "that was all there was" (#881).
- **Liveness is only asserted where it's observable** — attention derives from the durable Session registry. With no tmux, `process_alive: false` means "unknown", not "gone", and no phantom finding is raised. Bare `lf status` resolves the ambient wave by `LF_WAVE_ID`, and reports a *stale* context rather than "no wave" when the id isn't on this machine (#881).

## Operational notes

**The v0.11.0 release was wedged, and the pipeline that wedged it is fixed.** `v0.11.0` was tagged and dispatched, but `build-dmg` died on `FileNotFoundError: swift/.build/release/Loopflow` — the Mac product had been renamed to `LoopflowMac` and the DMG packager still copied the retired path. Everything downstream gates on `build-dmg`, so nothing published. The fix existed on `main`, but every job checks out the immutable tag, so a re-dispatch checked out the broken script and failed identically.

- **The packager reads the bundle** — `CFBundleExecutable` from `Info.plist` decides the copied name, covered by a regression test (#884).
- **A dispatch re-run can reach a tooling fix** — on `workflow_dispatch`, `build-dmg` checks the default branch into `.release-tooling/` and overwrites `scripts/` with it. Application source still comes from the tag; only packaging tooling floats to `main`. A tag pins *what ships*, not the script that ships it. Tag-push releases are unaffected, and a real release is dispatched from a tag freshly cut off `main`, so the copy only bites on a re-run — exactly when it should (#888).
- **An idempotent crate publish is not a failure** — Cargo now reports a re-publish as `already exists on crates.io index`; the workflow recognized only the older `already uploaded` wording and turned a successfully published crate into a red run. Both are accepted, every other publish error stays fatal, and Cargo output is always printed (#890).

**Schema.** This release ships one migration, `0.10.002_session_execution_context`, adding the pinned-context and abandon-intent columns to `task_sessions` and `project_sessions`. It applies on first run; existing rows keep NULL context and refuse to relaunch rather than guess. A patch release appends to the active `major.minor` namespace, which is why a 0.11.x migration is numbered `0.10.*` — the namespace is chosen when the migration is authored, and migrations authored after a minor bump begin the next one (#885).

## Small changes

- The `wave/README.md`, `swift/README.md`, and migration docs track the shipped contract rather than the intended one (#885, #878).
- `RegistryQuery` and the `wave_detail` DTO fixture carry the new evidence fields across Rust and Swift in lockstep (#881).

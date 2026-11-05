# e46a80: the store never ends a conversation

Task e46a80, infrastructure Wave. Jack Heart widened it on 2026-11-05: whatever
happens to lf's store, you keep using your LLM session. The store is where lf
records a conversation, never something the conversation depends on. Records lf
cannot write are logged to `logs/lf.log`; the person's turn is not lost.

Incident (2026-11-04 20:02 PST): `lf install` raised the reader floor with 177
old processes live. 23 idle Codex clients lost their relays within a second,
22 timed out reconnecting, and quitting on an old build failed with
`no such column: route`. Claude Sessions survived.

## Why Claude survived and Codex did not

Claude's data path is the TUI talking to Anthropic. lf touches it only through
hooks (separate `lf __context-block` processes; a store failure exits 1, which
Claude shows as a non-blocking hook notice and continues) and the PTY owner.
OpenCode and Pi are the same shape: lf observes, it does not carry.

Codex is different: lf's relay sits between the Codex TUI and the app-server
(`codex_connection.rs`). Any store error there was `return Err`, which closed
the websocket. The TUI then reconnected, and `serve_with_input` read the store
before the handshake, so every reconnect reset until Codex gave up.

The owner's input path has the same dependency for every provider: typed bytes
wait until `try_record_owner_input` commits, retrying forever. On 11-04 the
`processes` table was untouched, so typing kept working by luck.

## Decisions (Jack Heart, 2026-11-05 steer; implementation choices mine)

- Relay: every store failure other than `InvalidAuthority` is logged and the
  relay continues. `InvalidAuthority` still rejects (a replaced attachment is
  authority loss, not store loss). The pre-dispatch receipts for the saved
  initial input (`ThreadDispatched`, `InputDispatched`) still gate that send:
  without the receipt the input is retained for the next reconnect, as today.
  A person's own turn dispatches unfenced when the fence cannot be read.
- Reconnect: the relay accepts the handshake before anything else. Thread
  identity falls back from the store to the launch's shared memory
  (`NativeInput.thread`, set when Codex reports the thread), then to the
  connection's own `thread_id`.
- Owner input: a store *error* delivers the bytes unrecorded and holds the
  owner in memory until the native client ends; a *busy* store (`Ok(false)`)
  still waits 100 ms. LOO-709's "record before deliver" keeps its purpose (no
  idle ending while a draft exists) through the in-memory hold.
- Exit: `settle` and the launcher's return record best-effort. The exit code is
  the provider's own; the resume hint is already printed by the owner.
  `SessionCapture::finish` writes `terminal.json` and treats its store
  observation and attachment finish as best-effort.
- Hooks (Jack's 2026-11-05 steers): every `__context-block` call is best-effort.
  Claude: a store failure exits 1, which Claude Code treats as non-blocking
  (exit 2 would block); unchanged. Pi: the bash-tool guard no longer `exit`s the
  tool when lf fails, and `pi-event`, context refresh and steering failures
  notify once instead of shutting Pi down. OpenCode: the `context` hook catches
  a failed `__context-block` and sends the prompt without lf's context. The
  Claude task-update watcher warns and keeps polling instead of exiting.
- Quit timing: the owner records a handed-off exit on a thread and gives the
  terminal back after 800 ms with the provider's own exit code if records are
  still waiting on the store (15 s SQLite busy timeout per statement under an
  install's BEGIN EXCLUSIVE). The owner lingers up to 20 s for them to land.
- Hook store budget (Jack's steer 5): `SqliteStore::open_for_hook` waits at
  most 1 s (`HOOK_BUSY_TIMEOUT`) for a locked store; `__context-block`, Pi's
  `pi-event` and the checkout relocation use it. On any store error the hook
  emits the context block without steers, exits 0 and notes the gap once per
  native conversation. SessionStart with no store still selects the thread from
  the delivery file alone. Planning refresh at Stop also fits the 1 s budget.
- One recording-gap concept: `store::RecordingGap` (`note(what, error)` logs once
  per changed failure, `recovered()` once) is the owner's input path, the
  lifetime policy, the relay, the Claude task-update watcher and the hooks.
  Hooks are a fresh process per boundary, so theirs is `RecordingGap::marked`
  on the `<delivery>.store-gap` file, which carries the first and last miss.
  When a hook reaches the store again it writes one `store-gap` observation
  (`session_events`, source `store-gap:<first>:<last>`, payload first/last miss)
  before clearing the marker, so a Stop lf never recorded reads as unknown in
  the Session's history (Jack's steer). A failed write keeps the gap open.
- Pi and OpenCode match Claude: an lf they cannot run shows nothing in the
  conversation. Pi's extension appends one line per changed failure and one on
  recovery to `logs/pi-extension.log` (its UI toasts are gone); OpenCode's
  plugin does the same on its stderr, kept beside the capture, instead of a
  line on every prompt. The delivery file is `native_delivery_path`: it names
  Claude, Pi, OpenCode and Codex threads alike.
- Owner lifetime (Jack Heart, 2026-11-05, "kill it"): a removed, rebuilt or
  superseded lf executable ends nothing. The only endings are the person
  quitting, `lf resume` taking the owner over, `lf monitor rollover`, the
  detached limit, or an explicit end. `installation::superseded` stays for
  resume, rollover and doctor. The `Control` event lost its `successor`
  command; a viewer that loses input restores the terminal, prints the notice
  and leaves.
- Principle written where exercised: AGENTS.md harness guidance and
  `rust/loopflow/src/harness/README.md`. Wave memory (`.lf/wave/infrastructure/
  MEMORY.md`) carries the lifetime decision under "Idle owners end" and the
  traced 11-04 cause plus the middleman rule under "Upgrade continuity"; the
  shipped Session titles and Agent startup profiling detail retired to
  `a5000ef889` to stay under the 16000-token budget.

## Concept review (2026-11-05, headless)

The person's experience is one sentence: whatever lf's store does, your
conversation keeps accepting input, answering and quitting. Traced through the
change, the contract holds at each layer:

- Typed bytes: `try_record_owner_input` runs with a zero busy timeout, so a
  locked store answers `Ok(false)` at once and never stalls the serve loop; a
  failed store delivers unrecorded and the in-memory hold keeps the owner.
- A person's Codex turn: the dispatch fence is a file lock plus one read (WAL
  readers do not wait on an install's exclusive write), then an unfenced send if
  the fence cannot be read. Only the saved launch prompt waits for its receipt,
  and the TUI says so.
- Quit: the terminal returns in 800 ms. If the exit records never land, `lf
  resume` still continues the conversation: `owner_running` reads OS evidence,
  not the unrecorded `completed_at`. The demo's last step survives lost records.
- Lifetime: no store judgment or executable change ends an owner; a failed
  retention read retries at the next poll, and `lf resume` is the only thing
  that ends an idle superseded owner.

The review found four seams and the implement pass closed them, each now in
Decisions: one `store::RecordingGap` where four log-once bodies had grown; the
gap written into the Session's history as a `store-gap` observation at the next
successful boundary (Jack's steer), instead of a marker that cleared with a log
line while a missed Stop read as a live turn; `native_delivery_path` for the
file every native thread shares; and Claude as the reference behavior, so Pi's
toasts and OpenCode's per-prompt stderr line became one log line per changed
failure (Jack's quiet-terminal rule, 2026-10-21).

Kept after challenge: the saved launch prompt still waits for its receipt under
a broken store (double-dispatch protection from the codex-resume work outranks
sending it blind); `InvalidAuthority` still ends a relay (authority loss is not
store loss); exit records open the store with the full 15 s budget because the
terminal no longer waits on them.

No open product choice. Nothing here changes what the product is; each item
follows from Jack's steers and the written principle.

## Delete — do not maintain

All done; nothing left to remove. Gone: `transport::lifetime::warn_changed`,
`codex_connection::store_unavailable`/`store_recovered`, `context::store_gap`/
`store_recovered`/`gap_marker`, Pi's toast-raising `recordingGap` and its
task-update toasts, `codex_delivery_path` (now `native_delivery_path`), the
relay's fatal `return Err` arms on store errors, the `input_retry_tests` assertion that unrecorded input never
reaches the provider, `transport/lifetime.rs` `Executable`/`Change` and its
two-second grace, `Event::Control.successor`, `transport::follow`,
`journal::current_executable`, and the `sessions_end_when_their_lf_executable_goes`
and `successor_handoff_*` integration tests. Surviving behavior is pinned by
`sessions_keep_serving_when_their_lf_executable_goes`,
`control_handoff_restores_the_terminal_and_prints_the_notice`,
`idle_relay_survives_store_failure_and_accepts_a_reconnect`,
`settlement_without_a_store_keeps_the_providers_own_exit`,
`failed_input_store_delivers_the_queued_bytes_unrecorded`,
`a_recovered_gap_enters_the_sessions_history_once` and
`a_marked_gap_survives_processes_and_closes_with_its_file`.

## Not changed

- Old binaries. Sessions already on the 11-04 builds need `lf resume`.
- Why quitting left the terminal ignoring Ctrl-C/Ctrl-D is untraced. One
  candidate: the Codex launcher's `finish_session_attachment` runs under the
  Session fence file lock and SQLite's busy timeout with 177 processes
  contending; a blocked write there keeps the launcher alive after the client
  exits. Not proven here.
- `lf resume` after the store is restored is an ordinary launch; with the relay
  surviving, a client whose relay died is an old-build case.

## Demo (for Jack, needs the installed build)

Start `lf -a codex`, break the store under it (open it with a build whose floor
this one does not know), type, get a reply, quit: exit 0, prompt back, resume
hint. Restore the store; `lf resume` reopens it. Same with `lf -a claude`.
Not runnable headless here; fixtures below stand in.

## Checks

Implement pass (RecordingGap, gap observation, native_delivery_path, quiet Pi/OpenCode):
`cargo test -p loopflow --lib -- store::recording_gap:: lf::commands::context:: transport::lifetime:: transport::lifetime_tests:: transport::input_retry_tests:: harness::codex_connection:: harness::task_updates::` 34 passed;
`cargo test -p loopflow --test integration context_block_tests::` 2 passed;
`cargo test -p loopflow --test transport_tests -- sessions_keep_serving_when_their_lf_executable_goes control_handoff_restores_the_terminal_and_prints_the_notice a_terminal_launcher_becomes_its_provider_and_the_owner_records_its_exit native_exit_preserves_returning_launcher_results failed_native_exec_settles_once_without_launching_a_provider` 5 passed;
`node --check` on both JS hooks; `cargo fmt` and `cargo clippy --all-targets -D warnings` clean. Gate owns the affected suites.

Compress pass (one `GapSpan` in `RecordingGap`, one `Settling` slot in the owner, `Ending::confirm` answers yes/no): the same `--lib` filter 34 passed, the same `transport_tests` filter 5 passed, fmt and clippy clean.

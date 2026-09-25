# Keyed Ask recovery contribution

2026-09-25. Scope: `rust/loopflow/src/ops/human_session.rs` and this note.
The CLI, engine, Task controller, and integration remain owned by main.

## Contract and implementation

`ask_once(store, key, question, skill)` uses the existing Ask JSON record,
Session token, launch arguments, completion operation, and polling wait.
The exact boundary key becomes `ask_once_<SHA-256>`; neither the filename
nor the launcher name embeds the input key or any checkout path.

The first publication captures the active Run manifest, cwd, model, question,
optional skill, and existing Work resolution. Later calls preserve those facts.
An empty key is rejected. Recovery still requires a valid active Run manifest,
but does not reload a saved Ask's skill or replace its original question.

Concurrent callers serialize record creation and startup under the existing
Session launch lock. Lock acquisition runs on Tokio's blocking pool. They
reread after acquiring it, so only the winning record is published. Completed
records return their saved summary immediately. Waiting records join a published
native Run or an existing exact named tmux launcher; otherwise the ordinary
launcher starts the same Session. Failed launch leaves the record available
for retry and the error names its Session ID.

The private Ask record adds `retain_completed`, defaulting to false for old
records. Keyed Asks set it true. All keyed waiters can consume the same result,
including a restarted caller; the existing unresolved list and Session lookup
already exclude Completed records. Ordinary `lf ask` retains random IDs,
its prior background name, launch-failure cleanup, and completion cleanup.
There is no new Session kind, database table, or lifecycle store.

## Launch and native ownership trace

- `launch_ask` -> `start_durable_session` ->
  `engine/process.rs::start_home_session_with_env` selects the current Home
  control pair, passes the saved parent Run attribution, scrubs detached
  credentials, and starts `tmux new-session` running `lf session serve-ask`.
- The exact full-hash launcher name avoids truncating keyed identities.
  `tmux has-session -t =<name>` only prevents another background launcher while
  the provider is being published. It supplies no authority to signal a
  provider, infer native client death, or discard an existing Run binding.
- `serve_ask` takes the same launch lock. Its new under-lock reread check
  returns when another launch already published `session_run_id`, including
  when no native manifest can currently be read. It rejects a completed Ask.
  A delayed launcher cannot overwrite the published Run with a fresh provider.
- Existing `spawn_session_run` requires the child's Run binding, native session
  reference, and an owned active client before publication. It retains its
  timeout, exit checks, and kill-on-drop child handling.
- A recorded native Run is never automatically replaced by `ask_once`.
  `lf session open` retains ownership of reopening: `resume_native_run` uses
  the stored native session and original account through the existing util
  functions. `active_provider_clients` reads registered client receipts and
  checks process start evidence and harness identity; `replace_provider_clients`
  records the stop reason, waits for those clients, and refuses a failed move.
  The helper neither signals clients nor treats tmux absence as native death.
- Ask readiness and completion now serialize their record writes under the
  same lock. Readiness additionally rejects a completed record, preventing a
  late agent message from revising retained completion evidence. Readiness
  still cannot complete the Session. Completion still uses existing native
  stop protections and the ready summary.

## Verification

Four new tests use a temporary Home, restored environment, ephemeral SQLite,
and simulated background launch side effects (no tmux/provider/terminal):

1. Concurrent calls for one key share the result; another boundary remains
   independent. Completion is hidden from unresolved sessions and can be read
   again even when the requested skill is subsequently unavailable.
2. An injected first-launch failure preserves the question and Session ID;
   retry launches that same Ask and recovers its completed summary.
3. A saved native Run survives both a delayed serve command and an interrupted
   duplicate caller without a launcher or readable native manifest. Late
   readiness after completion is rejected.
4. Ordinary prompt-only Ask records with both new fields absent still decode,
   complete, and are removed after their caller receives the answer.

Focused verification passed: **12 tests passed, 0 failed**, including all four
new recovery tests and the existing native publication and skill-prompt tests:

```sh
env -u LF_CONTROL_HOME -u LF_CONTROL_DB_PATH -u LF_HOME -u LF_DB_PATH \
  -u LF_RUN_CONTEXT -u LF_RUN_ID -u LF_RUN_DIR \
  cargo test -p loopflow --lib ops::human_session::tests:: -- --nocapture
```

`rustfmt --check --edition 2021 --config skip_children=true` and the file-scoped
`git diff --check` passed. No full-suite pass is claimed.

Library Clippy passed with warnings denied (same isolated environment):

```sh
cargo clippy -p loopflow --lib -- -D warnings
```

This does not claim all-targets Clippy or a full CI pass.

An initial compile caught a missing test `Digest` import here, now fixed. Two
attempts were blocked by concurrent CLI edits (`Cli: Clone` without
`Commands: Clone`); the CLI owner removed that derive before the successful
run. The passing test build reported one out-of-scope unused `TempDir` import
in `lf/commands/flow.rs`.

## Review and limits

The existing Ask owner remains the sole record owner. Recovery never mutates
another Flow cursor or grants human gate approval. Tests inspect Session
records, returned summaries, independent keys, and retained native identity;
the launch simulator rejects duplicate names as the real named launcher does.
No factory trait, runtime injection interface, or second lifecycle was added.

This is not a live provider/desktop handoff proof. Existing process-publication
crash windows before `session_run_id` is written are not eliminated or claimed
as exactly-once provider execution by this helper. A published Run whose native
history is unavailable remains visible for explicit Session recovery rather
than being silently replaced. Retained keyed completion has no automatic
garbage collection; invocation retention policy belongs to integration.

No real human Session, provider, terminal, installation, commit, push, or PM
mutation was performed. A read-only `lf ps --json` observed machine state
while the shared build was blocked; it conferred no execution authority.

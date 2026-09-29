# Proposal: Exec entry and command return

2026-09-29 · LOO-298 · Prepared for Jack Heart. **Unapplied and uncompiled.**
Main owns integration, native usage, executable edits, builds and Git. This Run
writes only this artifact; private copies supplied the diff. The earlier
[Exec admission audit](research-exec-admission.md) remains the architecture map.

## Decision and exact scope

Integrate the bounded return-path repair below. It gives the executable entry
point ownership of ordinary command completion, removes both ordinary hard
exits, keeps exact CLI exit status, and closes the real-CLI/library admission
exemption. It does **not** satisfy every-process durable coverage yet.

The specific remaining conflict is store authorization before ordinary startup.
`SqliteStore::new` can initialize/migrate private data; `open_run_ledger_read_only`
cannot write. No inspected API establishes an already-permitted, compatible,
noninitializing **writable** sink before startup authorization and branch isolation.
An existing `LF_DB_PATH`, even with a compatible schema, does not supply that
permission: it may point to the installed target inspected by candidate preflight.
Selecting a second database, adding a bootstrap spool, or moving ordinary
admission before install/screenshot dispatch would invent another policy/owner.
This proposal does none of those things. Early paths do not attempt observation
writes at all; the stderr diagnostic makes the missing persistence explicit.
A later sink implementation must satisfy those constraints before the early-path
assertions below are deliberately replaced with complete coverage proofs.

This is the smallest coherent portion under the directive's explicit allowance
for retained coverage gaps, not a proposed waiver of the accepted contract.
No decision from Jack is invented or required to review/apply this portion.

## Proposed behavior

- `journal::with_process` captures entry time in memory before the existing gate,
  calls the CLI, waits for existing interrupt cleanup, then writes completion
  through the existing journal/Exec transaction. `PROCESS_STARTED_AT` is one
  process-local timestamp/entry marker, not another registry or durable owner.
- `admit_process` remains after successful ordinary authorization/isolation and
  cwd resolution. Existing `RunContext` retains the actual cwd and selected DB
  path so later environment/cwd changes cannot move completion to another sink.
  Start/finish still use the same `insert_run_event` transaction and Exec ID.
- The executable no longer relies on an outer `with_runtime` closure to finish.
  Nested wrappers are lifecycle no-ops in a CLI, including after admission
  failure; they cannot mint a replacement Exec to bypass that failure. Library
  invocations retain their current outer scope and no-process behavior.
- Clap returns instead of exiting. Help/version keep status 0; rejected arguments
  keep status 2. Its diagnostic prints once; print failure does not override its
  selected status, matching Clap's exiting path. Early startup errors keep status 1.
- `CommandExit` carries an already-rendered command status through `anyhow`,
  including through context wrapping. It is an error value, not a product record.
  SSH's remote exit 42 becomes the local OS/Exec exit 42. Empty release check
  keeps its existing message and exit 1. Ordinary errors keep `Error: {error:?}`.
  No extra generic error message follows these already-rendered exits.
- `LfEventFields.exit_code` is an internal emission parameter, not a new serialized
  DTO field. Exec retains the exact command status; Session/provider outcomes
  and Flow settlement are untouched. Interrupted/130 keeps unknown signal.
- A real CLI with no context, no persisted Exec in its conversation store, or no
  admitted conversation refuses driver acquisition before provider launch.
  The entry marker comes from code, never inherited environment. A malformed
  caller is not recast as a direct/root caller. Library calls outside the CLI
  keep their existing exemption. Existing driver/provider/native-turn fences
  remain unchanged; no caller token or native transport code changes.

One intentional visible addition is stderr
`Exec history unavailable: no ordinary store admission for this process` on
nonadmitted returns, even when tracing is disabled. Stdout/JSON and selected
status remain unchanged. This does not claim an early Exec row was written.
Ordinary ledger failures retain the existing warning policy; observation failure
still does not fail a mechanical command. Launch admission is stricter.

## Preserved counterexamples and boundaries

| Retained observation | How the patch handles it / remaining limit |
| --- | --- |
| Cached auth failed when admission required Git | Ordinary cwd admission stays after startup; no new repository discovery before parse/install/screenshot. Reuse Git-free auth/global-command proofs. |
| File journal moved under a subdirectory | Preserve existing independent root resolution; store actual command cwd. Existing obstructed-journal/subdirectory proof remains required. |
| Interrupted child wakes command before hook finishes, giving OS 1 versus Exec 130 | Entry owner waits on the existing cleanup mutex **before** ordinary terminal emission. Existing controlled Linux group-kill ordering test must pass unchanged. This is not proof for every possible signal arrival or SIGKILL. |
| SSH/release hard exits leave no terminal receipt | Return a status-bearing error; ordinary finalization writes the same status before main returns. No explicit process exit is added. |
| Malformed caller makes best-effort observation fail but library exemption allows launch | CLI driver acquisition now requires process evidence; public malformed-caller and selectively rejected Exec-write tests are included. No fallback to today's native turn. |
| Help/preflight/screenshot cannot depend on a usable ordinary Home | No early store open, migration, Git journal, process receipt, or extra sink. Existing command behavior executes; persistence is explicitly unavailable. |
| One actual process can cross wrappers and drive several boundaries | CLI nested wrappers cannot own completion or create new Execs. Existing direct-child and nested-wrapper tests remain applicable. |
| Failed native thread reuses the old tools' caller token | Not changed. Valid Codex decision retry and OpenCode decision integration remain red requirements. |

Remaining full-coverage gaps: persisted parser/startup/install/screenshot Execs;
public screenshot→supervisor ancestry; gate re-exec's original entry timestamp;
unknown signal/exit on abnormal termination; pre-admission termination diagnostics;
non-Unix delegation; native provider parity and dense discovery. Entry re-exec
replaces process memory before any row exists, so this cut creates no duplicate
Exec but does not preserve the earlier image's timestamp. No bootstrap observation
is fabricated from a later timestamp. Panics/abort/SIGKILL are not turned into
successful or failed receipts without evidence. The helper is intentionally
called once from main; it is not an arbitrary nested runtime API.

The new inspection proof seeds an existing unstarted Task and checks two real
read commands leave Started NULL. The SSH proof clears its environment, uses a
private Home, and replaces ssh/gh/security with scripts; it never prints the
forwarded preamble. The release proof uses TestRepo's local bare remote and
scripted gh. No credentials or remote service are part of either fixture.

## Review and verification status

Reviewed the exact patches against callers, store transaction, command dispatch,
Clap usage and interrupt hook ordering. Private-file `rustfmt --edition 2021
--config skip_children=true` parsed/formatted the proposed Rust; this is **syntax
and formatting only**, not type checking. A local text-only validator replayed
every unified-diff hunk against its saved original and compared the result with
the formatted proposal. It also located every hunk's exact old-side context in
the current worktree without writing there. These checks do not execute product
code. Main must still type-check, apply its source review and run the proofs.

No builds, product tests, providers, Home inspection through lf, PM calls, Git
mutations, installations, delegation, publication or process signaling occurred.
Read-only Git inspection identified the closing head; hashes below define the
actual source snapshot rather than assuming a clean or unchanged branch.

### Main's focused proof commands (not executed here)

Apply only the embedded unified diff; **do not copy private proposed source files**
over the worktree, because they predate main's concurrent native-usage work.
Use the existing isolated runner, after the normal resource preflight. No new
migration is proposed, so a canonical matrix is not required merely for this cut.

```sh
uv run python scripts/resource_envelope.py
uv run python .lf/tmp/cut-i/run.py exec-entry-proof cargo nextest run -p loopflow --test exec_ownership_tests --no-fail-fast -E 'test(parser_) | test(remote_command_status_is_the_local_exec_status) | test(empty_release_check_returns_through_exec_completion) | test(inspection_records_one_completed_exec_without_starting_work) | test(command_failure_records_the_process_result) | test(obstructed_file_journal_preserves_command_start_and_completion_in_sql) | test(interruption_records_the_exec_without_a_fabricated_signal_name)'
uv run python .lf/tmp/cut-i/run.py exec-admission-proof cargo nextest run -p loopflow --test session_cutover_tests --no-fail-fast -E 'test(inspection_execs_leave_an_existing_task_unstarted) | test(malformed_caller_cannot_use_library_agent_admission) | test(failed_exec_observation_cannot_admit_a_provider) | test(agent_admission_requires_the_store_before_provider_launch)'
uv run python .lf/tmp/cut-i/run.py exec-journal-proof cargo nextest run -p loopflow --lib --no-fail-fast -E 'test(journal::tests::)'
uv run python .lf/tmp/cut-i/run.py exec-early-boundaries cargo nextest run -p loopflow --test global_commands --test screenshot_tests --no-fail-fast
uv run python .lf/tmp/cut-i/run.py exec-entry-clippy cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Also run the existing exact interruption case on disposable Linux using the same
isolated-source/account procedure as `linux-interrupt-final.log`; macOS alone does
not execute its LD_PRELOAD-controlled ordering. Once the candidate is built in
that container, the exact Rust command is:

```sh
cargo nextest run -p loopflow --test exec_ownership_tests -E 'test(interruption_records_the_exec_without_a_fabricated_signal_name)' --no-fail-fast
```

Use the existing disposable installation harness for any later early-sink change;
`HOME` overrides do not isolate installation selection. This proposal changes no
install/preflight/screenshot dispatch, so do not infer full first-install proof
from its private-Home tests. Existing auth/startup diagnostics, direct/agent child
ancestry and valid ordinary launch proofs must survive integration; investigate
new failures before broadening a suite. No fixture pass has been claimed here.

## Source and patch identity

Closing hash check: `2026-09-29T09:42:46+00:00`. Observed closing HEAD before publication:
`ad8cb3a7ba0395ae8e033eb7e2afde8d77d84338`.

Proposed production delta: **+140/−41, net +99 lines** in six Rust
source files. Tests: +257/−0 in two integration suites. Method: line-based
unified diff against the saved source bytes, with all test-only files excluded
from production; no changed production hunk lies in a cfg(test) module.
No SQL, config, generated asset, harness transport or current test assertion is
removed. These are this proposal's counts, not whole-branch reduction.

| Proposed file | Added | Removed |
| --- | ---: | ---: |
| `rust/loopflow/src/bin/lf.rs` | 23 | 7 |
| `rust/loopflow/src/exec.rs` | 5 | 0 |
| `rust/loopflow/src/journal/mod.rs` | 91 | 30 |
| `rust/loopflow/src/lf/commands/ops/mod.rs` | 1 | 1 |
| `rust/loopflow/src/lf/commands/ssh.rs` | 5 | 3 |
| `rust/loopflow/src/run_record.rs` | 15 | 0 |
| `rust/loopflow/tests/exec_ownership_tests.rs` | 159 | 0 |
| `rust/loopflow/tests/session_cutover_tests.rs` | 98 | 0 |

Main drift: `run_record.rs` changed during preparation in native-usage recovery
and its conversation projection. Its original and closing hashes are below.
Both proposed driver-admission hunks still match the current source exactly with
line offsets. All other snapshotted files were unchanged at this closing check.
Preserve main's added native-usage logic; reconcile the two small admission hunks,
then rerun the admission proof on the combined bytes. Source may continue moving
after this artifact is published; recheck before applying.

Original SHA-256s (the diff's baseline; supplemental unchanged inputs establish
which contracts and owner code were read):

```text
c4890b24a6a7c95bc3f8632f2e942147be28d487cb2b8ea6b862e01017864950  rust/loopflow/src/bin/lf.rs
a01181f271b7d369625bf0fb42d6b969fb01c3d8a174cfe9571b9b2e44c406de  rust/loopflow/src/journal/mod.rs
c2b001c1443d12a8c3dd9bd0405f37efe8efabb20efd634d068e29648b6eb8ed  rust/loopflow/src/exec.rs
8a7fe75bf65bdb27ad6cf5a25e7e0ad7ca7d0eb8c1046693a29ea08635347e26  rust/loopflow/src/run_record.rs
1ba1161634d467aaf3767e291ad141ed192dcd5226d1b87593835db7b5248a01  rust/loopflow/src/lf/commands/ssh.rs
7e5150c9292a7fa0cabf082a574469ed75241baef71efa72a4e64c0445607bfb  rust/loopflow/src/lf/commands/ops/mod.rs
9fdad49927cbd392bcd773fd5e70a567c6bbb637c7d0721aef9f1a4eca6e8b02  rust/loopflow/tests/exec_ownership_tests.rs
d5696a60d530f2dcd2831d493c4bfa9b4c22ca46a2d755ef7c6e60c276268841  rust/loopflow/tests/session_cutover_tests.rs
879ddf4893c061c193fbebdebf7a406dbb91e62b05e27af242b0a3e97d377af9  rust/loopflow/src/engine/agent.rs
f8b81d43f2054e04b136b8de6d2b0667e3d3d336fb4321706139e200d765c34d  rust/loopflow/src/store/sqlite.rs
1e49f9041b0c1a1e382de59cd7fe9887505486ff9339153632ca4516d356a1f4  rust/loopflow/src/store/mod.rs
faed0c9a53ca4ea9580895c0d7815daa1b4f173443a85af6bfcb162040a716b2  rust/loopflow/src/machine_install.rs
66a7b7e1c1ac442ea5776962cb99ad6de0f247e485599ff0302740d1e1370451  rust/loopflow/src/lf/commands/install.rs
4553a4f8f3bfa518d7e0067a79b3cf468ee802c0df1cf1d7e0abed025aa7b6e4  rust/loopflow/src/lf/commands/screenshot.rs
ad969bf676ad18702b8150c1ece6d68c0b09a9bc821791ed43657f3ad2cc9241  TESTING.md
ea08aac56dfa4fd5d33d4804c6d56bfd33173d3df56ad9d767e8c61bf329e34c  scratch/research-exec-admission.md
49afb1c57c40f1720718136191ad2682945ceec5c19f21fcfde52c5d4ff66caa  scratch/data-model-one-table-per.md
1bfdf463cb265acc138237920141cb3b6613b7211e06d8d1ce0ba5e8628f804c  scratch/remaining-work.md
```

Closing differences:

```json
{
  "rust/loopflow/src/run_record.rs": {
    "snapshot": "8a7fe75bf65bdb27ad6cf5a25e7e0ad7ca7d0eb8c1046693a29ea08635347e26",
    "closing": "73564ba4b27e9c1dac520d804252fb383949b2d6a158e807249e4ab1253c5451"
  }
}
```

Proposed file SHA-256s describe **private full-file proposals**, not the integrated tree:

```text
3c2fa887fb7f6eab27ee6ec05e258b2d1702ad9419148e012914aee57a600233  rust/loopflow/src/bin/lf.rs
e3894024acf50aa424cf9ff3440260f4b08201fc3cf3ffbcb0718d19b70ffe90  rust/loopflow/src/exec.rs
87f03548f67758083f84f7cf9823dc115b0fb6bf269083ddb53c67615e598c8d  rust/loopflow/src/journal/mod.rs
afe4c238c81440337b2d48652122fcea60168f49a03e75dcfa8dc096d1565baa  rust/loopflow/src/lf/commands/ops/mod.rs
72da7c82a2da4a3aa2abf365df855ca326542971a1b368c7576d834eee25aa3c  rust/loopflow/src/lf/commands/ssh.rs
43775d56de082e9d46ba59f593cf08d48eea4b5eb7934a2ca04d1a14409bbb07  rust/loopflow/src/run_record.rs
6c6ee79326c761ab1ddd2f60ea5fa55b666055c48a34477323a30ef1b88b2f36  rust/loopflow/tests/exec_ownership_tests.rs
36ec29c3bf3f8304cd99d3d6b9ea4e5814f751f1d6b9ca6a298e2f41f026fdb2  rust/loopflow/tests/session_cutover_tests.rs
```

Unified diff SHA-256: `a8d9cca2ccf745ae9cc61a2ee73865a2ed89dbebed43813d71c205df32dc7233` (UTF-8, trailing newline included).

## Applicable unified diff

```diff
--- a/rust/loopflow/src/bin/lf.rs
+++ b/rust/loopflow/src/bin/lf.rs
@@ -1182,10 +1182,18 @@
     Ok((!report.trim().is_empty()).then_some(report))
 }
 
-fn main() -> anyhow::Result<()> {
-    let result = run();
-    loopflow::engine::agent::wait_for_interrupt_cleanup();
-    result
+fn main() -> std::process::ExitCode {
+    let result = journal::with_process(run);
+    let code = journal::command_exit_code(&result);
+    if let Err(error) = result {
+        if error
+            .downcast_ref::<loopflow::exec::CommandExit>()
+            .is_none()
+        {
+            eprintln!("Error: {error:?}");
+        }
+    }
+    std::process::ExitCode::from(code)
 }
 
 fn run() -> anyhow::Result<()> {
@@ -1210,7 +1218,14 @@
     // Reorder args so flags can appear after the skill name
     let args = reorder_args(normalize_ssh_args(std::env::args().collect()));
 
-    let mut cli = Cli::parse_from(args.clone());
+    let mut cli = match Cli::try_parse_from(args.clone()) {
+        Ok(cli) => cli,
+        Err(error) => {
+            let code = u8::try_from(error.exit_code()).expect("Clap exit status fits a byte");
+            let _ = error.print();
+            return Err(loopflow::exec::CommandExit(code).into());
+        }
+    };
     // Installation owns its promotion/recovery authority. In particular,
     // read-only candidate preflight must work before a first install settles.
     let bypasses_machine_startup_gate = matches!(&cli.command, Some(Commands::Install { .. }));
@@ -1305,7 +1320,8 @@
     // Exec admission records this process's cwd. Each operation resolves the
     // repository it needs after dispatch; machine inspection needs no Git.
     let directory = std::env::current_dir()?;
-    with_runtime(&directory, &args, || {
+    journal::admit_process(&directory, &args);
+    {
         let explicit_wave = cli
             .wave
             .as_deref()
@@ -1363,7 +1379,7 @@
             account_selection,
             inherited_account_lease,
         )
-    })
+    }
 }
 
 fn dispatch(
--- a/rust/loopflow/src/exec.rs
+++ b/rust/loopflow/src/exec.rs
@@ -5,6 +5,11 @@
 use crate::id::ExecId;
 
 pub const AGENT_CALLER_ENV: &str = "LF_AGENT_CALLER";
+
+/// A command already rendered its diagnostic and selected its process status.
+#[derive(Debug, thiserror::Error)]
+#[error("command exited with status {0}")]
+pub struct CommandExit(pub u8);
 
 /// Stable provenance installed in one provider conversation's tool environment.
 #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
--- a/rust/loopflow/src/journal/mod.rs
+++ b/rust/loopflow/src/journal/mod.rs
@@ -6,7 +6,7 @@
 use std::path::{Path, PathBuf};
 use std::process::Command;
 use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
-use std::sync::Arc;
+use std::sync::{Arc, OnceLock};
 
 use serde::{Deserialize, Serialize};
 use time::OffsetDateTime;
@@ -114,6 +114,10 @@
     static RUN_CONTEXT: RefCell<Option<RunContext>> = const { RefCell::new(None) };
 }
 
+// Only the executable entry point sets this. Library calls retain their own
+// outer with_runtime scope; inherited environment cannot opt into or out of it.
+static PROCESS_STARTED_AT: OnceLock<i64> = OnceLock::new();
+
 #[derive(Debug, Clone)]
 struct RunContext {
     run_id: TraceId,
@@ -123,6 +127,8 @@
     /// Time this command entered the runtime, independent of OS inspection.
     started_at: i64,
     process_started_at: Option<i64>,
+    cwd: PathBuf,
+    ledger_path: PathBuf,
     /// Serialized argv captured at run start so terminal rows name their work.
     command: Option<String>,
     /// File-journal directory. Written in any git checkout; None only when the
@@ -181,6 +187,7 @@
     pub index: Option<u32>,
     pub error: Option<String>,
     pub signal: Option<String>,
+    pub exit_code: Option<i32>,
 }
 
 #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
@@ -220,16 +227,57 @@
     }
 }
 
+/// Own the actual CLI return, without opening a store before command admission.
+///
+/// # Panics
+/// Panics if called more than once in the same process.
+pub fn with_process(run: impl FnOnce() -> anyhow::Result<()>) -> anyhow::Result<()> {
+    PROCESS_STARTED_AT
+        .set(OffsetDateTime::now_utc().unix_timestamp())
+        .expect("one lf entry point per process");
+    let result = run();
+    crate::engine::agent::wait_for_interrupt_cleanup();
+    if let Some(context) = current_context() {
+        finish_runtime(&context.cwd, &result);
+    } else {
+        // No compatible writable sink has been authorized on early paths.
+        // In particular, do not initialize a Home merely to log help/preflight.
+        eprintln!("Exec history unavailable: no ordinary store admission for this process");
+    }
+    result
+}
+
+pub fn command_exit_code<T>(result: &anyhow::Result<T>) -> u8 {
+    match result {
+        Ok(_) => 0,
+        Err(error) => error
+            .downcast_ref::<crate::exec::CommandExit>()
+            .map_or(1, |exit| exit.0),
+    }
+}
+
+pub(crate) fn is_cli_process() -> bool {
+    PROCESS_STARTED_AT.get().is_some()
+}
+
 pub fn with_runtime<T>(
     repo_root: &Path,
     command: &[String],
     run: impl FnOnce() -> anyhow::Result<T>,
 ) -> anyhow::Result<T> {
-    // Session completion and Flow recovery may enter this wrapper in-process.
-    // Only the outer command owns the actual lf process's lifecycle.
-    if current_context().is_some() {
+    // The executable owns admission and completion, even when observation
+    // failed. Nested library wrappers cannot mint another Exec to recover it.
+    if is_cli_process() || current_context().is_some() {
         return run();
     }
+    admit_process(repo_root, command);
+    let result = run();
+    finish_runtime(repo_root, &result);
+    result
+}
+
+/// Attach ordinary command observation after installation/data selection.
+pub fn admit_process(repo_root: &Path, command: &[String]) {
     let attribution = crate::work::wave::context::run_attribution(Some(repo_root));
     if let Some(failure) = attribution.failure.as_deref() {
         warn!(
@@ -250,25 +298,24 @@
             ..LfEventFields::default()
         },
     );
-    let result = run();
-    match &result {
-        Ok(_) => emit(
-            repo_root,
-            LfNode::Run,
-            LfEventType::Completed,
-            LfEventFields::default(),
-        ),
-        Err(error) => emit(
-            repo_root,
-            LfNode::Run,
-            LfEventType::Errored,
-            LfEventFields {
-                error: Some(format!("{error:#}")),
-                ..LfEventFields::default()
-            },
-        ),
-    }
-    result
+}
+
+fn finish_runtime<T>(directory: &Path, result: &anyhow::Result<T>) {
+    let code = command_exit_code(result);
+    emit(
+        directory,
+        LfNode::Run,
+        if code == 0 {
+            LfEventType::Completed
+        } else {
+            LfEventType::Errored
+        },
+        LfEventFields {
+            error: result.as_ref().err().map(|error| format!("{error:#}")),
+            exit_code: Some(i32::from(code)),
+            ..LfEventFields::default()
+        },
+    );
 }
 
 pub fn runs_root(worktree: &Path) -> PathBuf {
@@ -324,6 +371,7 @@
         return Ok(());
     }
 
+    let exit_code = fields.exit_code;
     let event = LfEvent {
         run_id: context.run_id.clone(),
         ts: if is_run_started {
@@ -351,7 +399,7 @@
     }
 
     let seq = next_seq();
-    ledger_insert(&context, &event, seq, repo_root);
+    ledger_insert(&context, &event, seq, repo_root, exit_code);
 
     if matches!(node, LfNode::Run)
         && matches!(
@@ -371,9 +419,15 @@
 }
 
 /// Best-effort write into the machine-grain SQLite ledger. Never fails the
-/// run: a locked or missing store degrades to a debug log line. Local-only —
+/// run: the first failure warns, and later failures log at debug. Local-only —
 /// the ledger never leaves the machine.
-fn ledger_insert(context: &RunContext, event: &LfEvent, seq: i64, repo_root: &Path) {
+fn ledger_insert(
+    context: &RunContext,
+    event: &LfEvent,
+    seq: i64,
+    repo_root: &Path,
+    exit_code: Option<i32>,
+) {
     let row = RunEventRow {
         run_id: event.run_id.as_str().to_string(),
         process_id: context.process_id.as_str().to_string(),
@@ -399,14 +453,14 @@
         error: event.error.clone(),
     };
 
-    match open_ledger() {
+    match SqliteStore::new(&context.ledger_path) {
         Ok(store) => {
-            let exit_code = match (event.node, event.event) {
+            let exit_code = exit_code.or(match (event.node, event.event) {
                 (LfNode::Run, LfEventType::Completed) => Some(0),
                 (LfNode::Run, LfEventType::Errored) => Some(1),
                 (LfNode::Run, LfEventType::Escalated) => Some(130),
                 _ => None,
-            };
+            });
             if let Err(err) = store.insert_run_event(
                 &row,
                 context.started_at,
@@ -493,6 +547,7 @@
         return Ok(Some(context));
     }
 
+    let ledger_path = ledger_db_path().map_err(std::io::Error::other)?;
     let main_repo = main_repo_root(repo_root).ok();
     let attribution = crate::work::wave::context::run_attribution(main_repo.as_deref());
     let wave_name = attribution.wave;
@@ -604,8 +659,13 @@
         process_id,
         parent_process_id,
         agent_caller,
-        started_at: OffsetDateTime::now_utc().unix_timestamp(),
+        started_at: PROCESS_STARTED_AT
+            .get()
+            .copied()
+            .unwrap_or_else(|| OffsetDateTime::now_utc().unix_timestamp()),
         process_started_at,
+        cwd: repo_root.to_path_buf(),
+        ledger_path,
         command: fields
             .command
             .as_ref()
@@ -645,6 +705,7 @@
             &event,
             interrupted.seq.fetch_add(1, Ordering::Relaxed),
             &directory,
+            Some(130),
         );
     });
     if let Err(error) = write_exec_process_receipt(&context) {
--- a/rust/loopflow/src/lf/commands/ops/mod.rs
+++ b/rust/loopflow/src/lf/commands/ops/mod.rs
@@ -1321,7 +1321,7 @@
 
     if changes.commits.is_empty() {
         eprintln!("No commits in the target area since the last tag.");
-        std::process::exit(1);
+        return Err(crate::exec::CommandExit(1).into());
     }
 
     let is_tty = std::io::stdout().is_terminal();
--- a/rust/loopflow/src/lf/commands/ssh.rs
+++ b/rust/loopflow/src/lf/commands/ssh.rs
@@ -303,12 +303,14 @@
         &extra_env,
     );
     let outcome = run_ssh(dest, port, forward_agent, broker.as_ref(), &preamble)?;
-    // `process::exit` skips destructors. Close the broker and remove its local
-    // socket before preserving a nonzero remote command's exact exit code.
+    // Release the broker before reporting the remote command's result.
     drop(broker);
     match outcome {
         SshOutcome::Success => Ok(()),
-        SshOutcome::CommandFailure(code) => std::process::exit(code),
+        SshOutcome::CommandFailure(code) => Err(crate::exec::CommandExit(
+            u8::try_from(code).expect("SSH command exit status fits a byte"),
+        )
+        .into()),
         SshOutcome::ConnectionFailure => {
             unreachable!("run_ssh returns transport failures as errors")
         }
--- a/rust/loopflow/src/run_record.rs
+++ b/rust/loopflow/src/run_record.rs
@@ -1682,6 +1682,11 @@
     /// used by its tools. A later driver transfer never rewrites this launch.
     pub(crate) fn claim_conversation_driver(&self) -> StoreResult<()> {
         let Some(exec_id) = crate::journal::current_exec_id() else {
+            if crate::journal::is_cli_process() {
+                return Err(StoreError::InvalidAuthority(
+                    "agent launch requires an admitted Exec; command observation failed".into(),
+                ));
+            }
             // Library callers outside an actual lf process have no Exec to name.
             return Ok(());
         };
@@ -1690,7 +1695,17 @@
             return Ok(());
         }
         let store = row_store(&capture.dir)?;
+        if crate::journal::is_cli_process() && !store.process_is_recorded(exec_id.as_str())? {
+            return Err(StoreError::InvalidAuthority(
+                "agent launch requires an admitted Exec in the conversation store".into(),
+            ));
+        }
         let Some(session) = store.session_for_run(&capture.manifest.run_id)? else {
+            if crate::journal::is_cli_process() {
+                return Err(StoreError::InvalidAuthority(
+                    "agent launch requires an admitted conversation".into(),
+                ));
+            }
             return Ok(());
         };
         let expected = store.session_driver(&session.id)?;
--- a/rust/loopflow/tests/exec_ownership_tests.rs
+++ b/rust/loopflow/tests/exec_ownership_tests.rs
@@ -1,6 +1,7 @@
 //! Command observation is durable even when no agent work starts.
 #![cfg(unix)]
 
+use std::os::unix::fs::PermissionsExt;
 use std::path::Path;
 use std::process::{Child, Command};
 use std::time::{Duration, Instant};
@@ -53,6 +54,164 @@
         )
         .unwrap();
     assert_eq!(work, 0, "inspection must not reserve agent or Task work");
+}
+
+#[test]
+fn parser_returns_exact_status_without_admitting_an_early_store() {
+    for (args, code) in [
+        (vec!["--help"], 0),
+        (vec!["--version"], 0),
+        (vec!["session", "list", "--definitely-not-a-flag"], 2),
+    ] {
+        let home = tempfile::tempdir().unwrap();
+        let database = home.path().join("loopflow.db");
+        // An incompatible existing target must not be opened or repaired for help.
+        std::fs::write(&database, b"retained incompatible store").unwrap();
+        let output = command(home.path(), home.path(), &args)
+            .env("PATH", "")
+            .env("RUST_LOG", "off")
+            .output()
+            .unwrap();
+        assert_eq!(output.status.code(), Some(code), "{output:?}");
+        assert!(String::from_utf8_lossy(&output.stderr)
+            .contains("Exec history unavailable: no ordinary store admission"));
+        assert_eq!(
+            std::fs::read(&database).unwrap(),
+            b"retained incompatible store"
+        );
+        assert_eq!(std::fs::read_dir(home.path()).unwrap().count(), 1);
+        if code == 0 {
+            assert!(!output.stdout.is_empty(), "{output:?}");
+        } else {
+            assert!(String::from_utf8_lossy(&output.stderr).contains("unexpected argument"));
+        }
+    }
+}
+
+#[tokio::test]
+async fn parser_does_not_treat_a_compatible_store_as_write_permission() {
+    let home = tempfile::tempdir().unwrap();
+    let database = home.path().join("loopflow.db");
+    let store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
+        .await
+        .unwrap();
+    drop(store);
+    let original = std::fs::read(&database).unwrap();
+    let output = command(home.path(), home.path(), &["--help"])
+        .env("PATH", "")
+        .output()
+        .unwrap();
+    assert_eq!(output.status.code(), Some(0), "{output:?}");
+    assert_eq!(std::fs::read(&database).unwrap(), original);
+    let conn = rusqlite::Connection::open(&database).unwrap();
+    let count: i64 = conn
+        .query_row("SELECT count(*) FROM execs", [], |row| row.get(0))
+        .unwrap();
+    assert_eq!(
+        count, 0,
+        "this bounded cut explicitly leaves early persistence unavailable"
+    );
+}
+
+#[test]
+fn remote_command_status_is_the_local_exec_status() {
+    let home = tempfile::tempdir().unwrap();
+    let repo = TestRepo::new();
+    let bin = home.path().join("bin");
+    std::fs::create_dir(&bin).unwrap();
+    // No real credential CLI, Keychain reader or remote transport participates.
+    for (name, script) in [
+        ("ssh", "#!/bin/sh\ncat >/dev/null\nexit 42\n"),
+        ("gh", "#!/bin/sh\nexit 1\n"),
+        ("security", "#!/bin/sh\nexit 1\n"),
+    ] {
+        let path = bin.join(name);
+        std::fs::write(&path, script).unwrap();
+        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
+    }
+    let output = command(
+        home.path(),
+        repo.path(),
+        &["ssh", "proof@example.invalid", "catalog"],
+    )
+    .env_clear()
+    .env("HOME", home.path())
+    .env("LF_HOME", home.path())
+    .env("LF_DB_PATH", home.path().join("loopflow.db"))
+    .env(
+        "PATH",
+        format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", bin.display()),
+    )
+    .output()
+    .unwrap();
+    assert_eq!(output.status.code(), Some(42), "{output:?}");
+    assert!(
+        !String::from_utf8_lossy(&output.stderr).contains("Error:"),
+        "{output:?}"
+    );
+    assert_recorded_exit(home.path(), 42);
+}
+
+#[test]
+fn empty_release_check_returns_through_exec_completion() {
+    let repo = TestRepo::new();
+    let tagged = Command::new("git")
+        .args(["tag", "v0.9.0"])
+        .current_dir(repo.path())
+        .output()
+        .unwrap();
+    assert!(tagged.status.success(), "{tagged:?}");
+    let home = tempfile::tempdir().unwrap();
+    let bin = home.path().join("bin");
+    std::fs::create_dir(&bin).unwrap();
+    let gh = bin.join("gh");
+    std::fs::write(&gh, "#!/bin/sh\ncase \"$1 $2\" in '--version ') exit 0;; 'pr list') echo '[]'; exit 0;; esac\nexit 1\n").unwrap();
+    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
+    let output = command(home.path(), repo.path(), &["release", "check"])
+        .env(
+            "PATH",
+            format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", bin.display()),
+        )
+        .output()
+        .unwrap();
+    assert_eq!(output.status.code(), Some(1), "{output:?}");
+    let stderr = String::from_utf8_lossy(&output.stderr);
+    assert!(
+        stderr.contains("No commits in the target area since the last tag."),
+        "{output:?}"
+    );
+    assert!(!stderr.contains("Error:"), "{output:?}");
+    assert_recorded_exit(home.path(), 1);
+}
+
+fn assert_recorded_exit(home: &Path, code: i32) {
+    let conn = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
+    let rows: Vec<(String, Option<i32>, bool, Option<String>)> = conn
+        .prepare("SELECT outcome,exit_code,completed_at IS NOT NULL,signal FROM execs")
+        .unwrap()
+        .query_map([], |row| {
+            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
+        })
+        .unwrap()
+        .collect::<Result<_, _>>()
+        .unwrap();
+    assert_eq!(rows, vec![("failed".into(), Some(code), true, None)]);
+    let events: Vec<String> = conn
+        .prepare("SELECT event FROM run_events WHERE node='run' ORDER BY seq")
+        .unwrap()
+        .query_map([], |row| row.get(0))
+        .unwrap()
+        .collect::<Result<_, _>>()
+        .unwrap();
+    assert_eq!(events, ["started", "errored"]);
+    let work: (i64, i64) = conn
+        .query_row(
+            "SELECT (SELECT count(*) FROM agent_sessions), (SELECT count(*) FROM flow_sessions)",
+            [],
+            |row| Ok((row.get(0)?, row.get(1)?)),
+        )
+        .unwrap();
+    assert_eq!(work, (0, 0));
 }
 
 #[test]
--- a/rust/loopflow/tests/session_cutover_tests.rs
+++ b/rust/loopflow/tests/session_cutover_tests.rs
@@ -1135,6 +1135,104 @@
         std::fs::set_permissions(&self.directory, std::fs::Permissions::from_mode(0o755)).unwrap();
         rusqlite::Connection::open(self.directory.join("loopflow.db")).unwrap()
     }
+}
+
+#[test]
+fn inspection_execs_leave_an_existing_task_unstarted() {
+    let fixture = Fixture::new(false);
+    let task = support::register_unrun_task(
+        fixture.home.path(),
+        fixture.repo.path(),
+        "inspection-only",
+        &fixture.repo.head_sha(),
+    );
+    for args in [
+        vec!["session", "list", "--task", "INF-123", "--json"],
+        vec!["usage", "--task", "INF-123", "--json"],
+    ] {
+        let output = fixture.run(&args);
+        assert!(output.status.success(), "{output:?}");
+    }
+    let started: Option<i64> = fixture
+        .db()
+        .query_row(
+            "SELECT started_at FROM tasks WHERE id=?1",
+            [task.task.id.as_str()],
+            |row| row.get(0),
+        )
+        .unwrap();
+    assert_eq!(started, None);
+    assert_eq!(fixture.count("agent_sessions"), 0);
+    let completed: i64 = fixture.db().query_row(
+        "SELECT count(*) FROM execs WHERE outcome='succeeded' AND exit_code=0 AND completed_at IS NOT NULL",
+        [], |row| row.get(0),
+    ).unwrap();
+    assert_eq!(completed, 2);
+}
+
+#[test]
+fn failed_exec_observation_cannot_admit_a_provider() {
+    let fixture = Fixture::new(false);
+    let initialized = fixture.run(&["session", "list", "--all", "--json"]);
+    assert!(initialized.status.success(), "{initialized:?}");
+    // Fail just the process observation. The conversation store remains writable.
+    fixture
+        .db()
+        .execute_batch(
+            "CREATE TRIGGER refuse_fixture_exec BEFORE INSERT ON execs
+         BEGIN SELECT RAISE(ABORT, 'fixture refuses Exec observation'); END;",
+        )
+        .unwrap();
+    for args in [
+        LAUNCH.as_slice(),
+        &["-b", "--model", "opencode", ":", "Tidy the parser"],
+    ] {
+        let output = fixture.run(args);
+        assert_eq!(output.status.code(), Some(1), "{output:?}");
+        assert!(
+            fixture.launches().is_empty(),
+            "a writable Session store cannot replace Exec admission"
+        );
+    }
+    assert_eq!(
+        fixture.count("execs"),
+        1,
+        "retain the earlier inspection only"
+    );
+}
+
+#[test]
+fn malformed_caller_cannot_use_library_agent_admission() {
+    let fixture = Fixture::new(false);
+    for args in [
+        LAUNCH.as_slice(),
+        &["-b", "--model", "opencode", ":", "Tidy the parser"],
+    ] {
+        let output = fixture
+            .command(args)
+            .env("LF_AGENT_CALLER", "not-json")
+            .output()
+            .unwrap();
+        assert_eq!(output.status.code(), Some(1), "{output:?}");
+        assert!(
+            String::from_utf8_lossy(&output.stderr)
+                .contains("agent launch requires an admitted Exec"),
+            "{output:?}"
+        );
+        assert!(
+            fixture.launches().is_empty(),
+            "no provider starts without process admission"
+        );
+    }
+    // A diagnostic command is still usable. Invalid provenance never becomes
+    // a fabricated root/direct Exec merely to make logging succeed.
+    let output = fixture
+        .command(&["session", "list", "--all", "--json"])
+        .env("LF_AGENT_CALLER", "not-json")
+        .output()
+        .unwrap();
+    assert!(output.status.success(), "{output:?}");
+    assert_eq!(fixture.count("execs"), 0);
 }
 
 #[test]
```

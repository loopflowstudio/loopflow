# LOO-825: conversation opening

Status: implemented and focused checks pass. Gate owns the affected suites and
isolated native-host acceptance.

The Task brief delegates the remaining product decisions:

> Finish the remaining product details autonomously in the written design: the scope of already-open detection (same terminal versus other local hosts/Machines), repository/Wave opening and workspace command placement, and the result for a plain terminal without sibling-tab support. Do not silently change the requested mode.

## Direction for implementation

- `lf open` resolves one scope's primary Session, or an exact `--session`.
  Task, Wave and repository selection all open that scope's conversation.
- Direct CLI defaults to `--here`; agent calls default to `--tab`.
  `--here`, `--tab` and `--focus` are mutually exclusive. Desktop navigation
  focuses its existing view. Agent `--here` fails with a human-runnable command.
- Ordinary terminal opening reports a known existing view and offers `--focus`.
  Match Session/view identity across local hosts on the presentation Machine;
  do not enumerate other Machines. Report a known remote viewer without taking
  input. Explicit `resume` remains the way to move input.
- `--focus` creates nothing: no live view means `Not open`; an unfocusable
  view gets its location and a failure. A closed Session can reopen from history
  through ordinary `open` without replaying its prompt or moving its Graph.
- `--tab` without a supporting host fails and supplies a command to run manually.
  No implicit Desktop launch, host fallback or mode change.
- `lf workspace` owns explicit layouts, repository/Wave overviews, `--available`,
  `--no-focus` and native workspace following. Ordinary `open` ignores layouts.
  Existing standalone workflow and change inspection stay `lf task view` and
  `lf diff`. Desktop's `open --diff`, file presentation and link output remain.
- A workspace Conversation pane uses the shared Session resolver and opening
  rules. Overviews and arrivals start no agents. Already-open conversations
  retain input; requested conversations never turn into workflow panes.
- Conversation success requires attachment. Failures return to the caller,
  retain readable destination diagnostics, and allow the same command to retry
  after rechecking live attachment. Keep successful workspace tools on partial
  failure. Use existing Session/view records, not another workspace registry.

## Docs to read in the implementation step

- `docs/workspaces.md` — opening contract, modes, workspace entry point, recovery.
- `docs/surfaces.md` — host and Desktop entry points.
- `docs/sessions.md` — opening versus deliberate input takeover with resume.
- `docs/config.md` — layout scope.
- `README.md` — command table.
- `rust/loopflow/src/builtins/task/skill/capture-tasks.md` — agent opening guidance.
- `rust/loopflow/src/builtins/ops/skill/init.md` — explicit Desktop entry point.

Keep CLI help and command examples in agreement with these docs when implementing.
The command reference's generated sections remain generated from Clap.
Attachment/host tests must use isolated fixtures, including same-Rex identity,
agent own-Session opening, focus present/absent, closed Sessions and failed attach.
The Task brief's PR #1876 reconnect work remains separate.

## Remaining verification

Gate: affected Rust suites (`open`, `desktop`, CLI discovery, Wave resolution,
Work opening, transport and the CLI binary), documented commands, architecture,
clippy and the broader Swift suite. Run isolated Rex/Herdr acceptance there;
never use a live host for those fixtures.

- `cargo test -p loopflow --test integration work_open_tests::` — compression run: 28 passed, four 2s host-request timeouts, one invalid new regression fixture (replaced), two native-host checks deferred to gate.
- Eight serial rechecks passed: caller identity, available conversations, links, focus, Rex display/fresh tabs/following and tmux.
- `target/debug/deps/integration-e6cabc0454f7ba0f work_open_tests::workspace_attachment work_open_tests::cmux_launches work_open_tests::configured_task_layout --test-threads=1` — three passed on the final code; the parallel run hit Rex/cmux 2s timeouts. Gate should recheck host timing.
- `cargo test -p loopflow --lib -- lf::commands::open:: server::schema::tests::the_checked_in_schema_is_what_describe_prints` — 21 passed after compression; linker warned that `__eh_frame` exceeds 16MB. Earlier parser (40), Wave registry and conversation receipt checks passed.
- `cargo test -p loopflow --test transport_tests conversation_open_continues_closed_history_without_replaying_the_request` — passed; the direct opening/non-takeover check also passed.
- `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter tasklessSessionLink` — app built; repository and Wave cases passed without WindowServer access.

Earlier file presentation of the seven docs returned "No Desktop Files view
attached; nothing opened." No fallback launched.

## Delete — do not maintain

These paths are removed; no deletion targets remain.

- Normal `open` dispatch into `workspace::run`; layouts, arrivals and batch
  selection move exclusively to `workspace`; default `desktop view` uses that
  same arrangement entry point.
- `Opening::starting`/`record` and `STARTING`: a remembered destination is not
  attachment. Keep only the concurrent opening lock.
- `open_terminal` graphical launch/fallback and implicit Desktop routing.
- `primary::TaskResume`/`task_resume`, `current_conversation` suppression and
  `Resume::Detached`/caller suppression:
  use the shared non-replacing Session connection for every Conversation pane.
- `DesktopBatch`, `Opened`, the remaining `Resume` policy enum and attachment
  collections: one opening has one destination; a Task layout has one Session pane.
- Tests asserting fresh layouts for normal opening, implicit focus, cached
  starting success or workflow substitution. Keep layout tests on `workspace`;
  replace conversation coverage with the new modes and attachment receipts.

Implementation uses `select_connection` and `--start-if-free`, which already
own primary selection, closed-history continuation and atomic non-takeover.
A transient launch acknowledgement records the transport's input-ready Return;
workspace/tab creation alone cannot satisfy it. No workspace registry is added.

The compression pass removes the leftover Desktop batch receipt registry: each
conversation confirms its own attachment and an arrangement is one URL. Normal
selection returns one target. Task layouts permit exactly one
Session pane, so layout rendering owns one optional attachment, without a vector,
`RefCell`, or parsing synthetic argv. The failure regression checks that the
created workspace destination and tools remain available.

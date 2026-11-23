# Settings redesign · LOO-1066

> Visual redesign of the settings pane. make it more like a standard Apple Settings pane, sort of like Rex

Direction on record: [demo-notes.md](demo-notes.md) (verbatim feedback from Jack Heart during the live demo of PR #2033), [design.md](design.md), [explore.html](explore.html), [qa.html](qa.html), and [concept-review.html](concept-review.html). User docs: [Surfaces](../docs/surfaces.md#settings), [Configuration](../docs/config.md#inspect-effective-settings), [Connect clients](../docs/server.md#phone-access), and [Loopflow for macOS](../swift/README.md).

## Remaining slice: Self-healing Settings observation

**Experience & model.** Settings is a live window on the Machine’s configuration files, just like the main Desktop window (`WorkModel`), `Appearance`, `Mobile`, and `Machines`. When the local `lf` server restarts (`lf update`, `lf server restart`, sleep/wake, or startup right as Settings opens), both `personal` and `project` scopes keep the last good reading visible, pause edits (`editsDisabled == true`), show the error banner, and **reconnect automatically** once the server answers—without requiring manual **Refresh** in each scope or closing and reopening Settings.

**Code change (`swift/LoopflowMac/Services/ScopedSettingsModel.swift`).**
- Replace the one-shot `observe()` body with a `while !Task.isCancelled && observation == id` loop with bounded exponential backoff (`1s → 2s → 4s → … → 30s`, resetting to `1s` once a healthy frame with `frame.body != nil` arrives, matching `WorkObservationRetry`).
- Fetch `catalogue` inside the loop when `catalogue.isEmpty` or after a reconnect so cold-open during server startup and mid-session `lf update` restarts both recover cleanly.
- On every failed attempt or ended stream, keep the last good scope visible via `reading = reading.failed(...)`, clear `self.stream = nil`, call `finishConfirmations(through: .max)` so no in-flight `confirmReading()` hangs, call `await stream.cancel()`, and then sleep the backoff delay before retrying.
- Keep `refresh()` and `work.didSet` immediate: changing `observation = UUID()` cancels any in-flight backoff sleep via `.task(id: observation)` and connects immediately with fresh 1s backoff.
- Leave YAML syntax errors untouched: the server delivers those as `SubscriptionFrame(body: nil, unavailable: ...)` without ending the stream, so the reconnect loop never churns while a file is broken on disk.

**Verification.**
- Add a deterministic test in `swift/LoopflowTests/SettingsWorkflowTests.swift` using `LocalServerFixture`: start `ScopedSettingsModel.observe()`, confirm initial `.available` reading, call `await server.stop()`, wait for `reading.failure != nil` (`editsDisabled == true`), restart the server with `try await server.status()`, wait for `reading.failure == nil` and `editsDisabled == false` **without** calling `model.refresh()`, and apply an edit to confirm writes work again.
- Re-run `scripts/test_desktop.sh --filter 'SettingsWorkflowTests|ConfigurationSettingsTests|ContextSettingsWorkflowTests|ThemeWorkflowTests|DesignDisciplineTests'`.

## Already built on this branch (keep intact)

- **Policy B sidebar & window:** 6 personal panes (`General`, `Appearance`, `Shortcuts`, `Agents`, `Machines`, `Mobile`) + 1 `Project & Wave Config` pane, monochrome SF Symbols, 2 unselectable dividers, capsule search bar, unused sidebar-toggle toolbar removed, independent `personal` / `project` scopes, deduplicated search errors, `Open ~/.lf/config.yaml` and `Open file…`.
- **Settings rows & targeting (pass 1 resolved):** Uniform candidate filtering + `definition.unset` fallbacks for `agent`, `default_agent`, and `agents`; `In effect` provenance gated to declared or non-builtin participating keys with the selected file path shown once beside `Open file…`; human enum choice labels; `Explicit null` shown only when already declared; instant checked Reset + human-titled Undo for simple controls (including emptied number fields) with previewed `Reset…` kept for `.yaml` editors; `Skill or Graph step` disclosure; shared `SettingsWork.scope(for:)` predicate for both form filtering and Search navigation; direct `Configuration file` path in `Appearance`.
- **Rust backend:** Bulk `config.scope` read/subscription, `observe::invalidate_path` synchronous cache invalidation + `read_stream::wake_file_readers()`, config locks isolated under `$LF_HOME/locks/config/` so edits leave `git status` clean, `config.preview` / `config.edit` checked YAML writer preserving comments and 3-way absent/`null`/value semantics, `LandMethod` enum, `Definition.unset` wire field, `config.promote` atomic/resumable 2-file move + idempotent `Action::Dismiss`, `machine.status` concurrent SSH probes, and documented `lf config scope` CLI command.
- **Desktop surfaces:** `ScopedSettingsModel`, `SettingsUndo`, `AppearanceStore` instant-apply + Undo + staged `ThemeEditor`, `AutosaveToggle`, shared `KeyboardShortcutsView`, lifted `MobileSetup`, and `MachineConnections`.

## Prior verification on record

- `cargo test -p loopflow-execution --lib config::` passed 114 tests, including `a_settings_write_leaves_only_the_settings_file_in_the_checkout` and shared catalog fixture.
- `cargo test -p loopflow --test integration documented_commands::` passed all four checks.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed.
- `scripts/test_desktop.sh --filter 'SettingsWorkflowTests|ConfigurationSettingsTests|ContextSettingsWorkflowTests|ThemeWorkflowTests|DesignDisciplineTests'` passed headless suites; compact-window layout diagnostic awaits an unlocked display at visual review.
- QA round 4 ([qa.html](qa.html)) drove the branch app on a locked screen and verified Agent Policy fallbacks, emptied number field Reset, gated `In effect`, clean checkout `git status`, and broken-YAML live recovery. Visual review of sidebar chrome and scoped forms on an unlocked display remains for the human demo.

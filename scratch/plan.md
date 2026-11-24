# Settings redesign · LOO-1066

> Visual redesign of the settings pane. make it more like a standard Apple Settings pane, sort of like Rex

Direction on record: [demo-notes.md](demo-notes.md) (verbatim feedback from Jack Heart during the live demo of PR #2033), [design.md](design.md), [explore.html](explore.html), [qa.html](qa.html), and [concept-review.html](concept-review.html). User docs: [Surfaces](../docs/surfaces.md#settings), [Configuration](../docs/config.md#inspect-effective-settings), [Connect clients](../docs/server.md#phone-access), and [Loopflow for macOS](../swift/README.md).

## Compact window review

Reviewed the 760 × 592 pt Settings window on an unlocked display. All seven sidebar labels fit. The project form shows the checkout, destination and agent source in full. Searching for `merge_method` reveals its row with the full default value. See the [window captures](qa.html#compact).

`LOOPFLOW_NATIVE_TESTS=1 swift test --package-path swift --filter searchScrollsBetweenProjectSectionsInACompactWindow` passed on the unlocked Mac.

## Remaining

- Implement the two Settings interactions below, then repeat the focused view and scope checks.
- Jack Heart's look at the second beauty pass ("then second lets take a pass to make it more beautiful") on the Dev build, via the tour skill (`scratch/tour.yaml`).
- Gate: publish so CI runs the full suites on PR #2033 (current checks were skipped).

## Open the declaration and reset a picker

1. **Open the existing Wave document.** Start with a shared Wave whose only file is `GOAL.md` with `agent: codex` in frontmatter. A fresh `config scope` reported that declaration as present, identified its selected candidate in the goal, and returned a missing `config.yaml` as the shared file path. Desktop's Open file… follows that missing path. The first slice is a scope-to-view test: opening leads to the goal without writing anything; after the first changed save it leads to the populated YAML file. A no-op keeps the goal active.
   - In `rust/loopflow-execution/src/config/files.rs`, carry the existing `settings_source` result through the document read into `engine/config/scope.rs`. Keep `ScopeFile.path` as the save destination. Expose the actual read path on that file in the Rust/Swift DTO and update its round-trip and schema fixtures. Derive it from the selected document, independently of which candidate wins. An absent private file must never open another owner's file.
   - In `swift/LoopflowMac/Services/ConfigurationScope.swift` and `Views/ScopedSettings.swift`, Open follows the read path; show the first-edit save path only when it differs. Update the file-opening passage in `docs/surfaces.md`. Keep the same declaration identity, expected entries, write lock, and Undo receipt through the transition. Check a no-op, a real first write and an externally created YAML file; keep the existing source-race tests.
2. **Give a saved picker choice one Reset.** With shared `merge_method: squash` and private `merge_method: rebase`, the picker labels its selectable absent tag “Rebase (from My repository settings)”. Removing the private declaration produced `squash` from the shared file in a fresh CLI run. In `Views/ScopedSettingRow.swift`, render the sourced absence tag only while the slot is absent. Reset below the picker already performs the checked write. Cover the actual picker, Reset and Undo in `swift/LoopflowTests/SettingsWorkflowTests.swift`, including the inherited label after Reset. Keep explicit null selectable only when already present and keep the separate “In effect” reading for shadowed declarations.

## Settings observation

Personal and project scopes keep their last reading visible and pause edits while disconnected. Each retries the catalogue and scope with a 1–30 second delay that resets after a healthy reading. Refresh and scope selection start a fresh observation immediately. A file error stays on its live subscription.

## Already built on this branch (keep intact)

- **Policy B sidebar & window:** 6 personal panes (`General`, `Appearance`, `Shortcuts`, `Agents`, `Machines`, `Mobile`) + 1 `Project & Wave Config` pane, monochrome SF Symbols, 2 unselectable dividers, capsule search bar, unused sidebar-toggle toolbar removed, independent `personal` / `project` scopes, deduplicated search errors, `Open ~/.lf/config.yaml` and `Open file…`.
- **Settings rows & targeting (pass 1 resolved):** Uniform candidate filtering + `definition.unset` fallbacks for `agent`, `default_agent`, and `agents`; `In effect` provenance gated to declared or non-builtin participating keys with the selected file path shown once beside `Open file…`; human enum choice labels; `Explicit null` shown only when already declared; instant checked Reset + human-titled Undo for simple controls (including emptied number fields) with previewed `Reset…` kept for `.yaml` editors; `Skill or Graph step` disclosure; shared `SettingsWork.scope(for:)` predicate for both form filtering and Search navigation; direct `Configuration file` path in `Appearance`.
- **Rust backend:** Bulk `config.scope` read/subscription, `observe::invalidate_path` synchronous cache invalidation + `read_stream::wake_file_readers()`, config locks isolated under `$LF_HOME/locks/config/` so edits leave `git status` clean, `config.preview` / `config.edit` checked YAML writer preserving comments and 3-way absent/`null`/value semantics, `LandMethod` enum, `Definition.unset` wire field, `config.promote` atomic/resumable 2-file move + idempotent `Action::Dismiss`, `machine.status` concurrent SSH probes, and documented `lf config scope` CLI command.
- **Desktop surfaces:** `ScopedSettingsModel`, `SettingsUndo`, `AppearanceStore` instant-apply + Undo + staged `ThemeEditor`, `AutosaveToggle`, shared `KeyboardShortcutsView`, lifted `MobileSetup`, and `MachineConnections`.

## Prior verification on record

- `cargo test -p loopflow-execution --lib config::` passed 114 tests, including `a_settings_write_leaves_only_the_settings_file_in_the_checkout` and shared catalog fixture.
- `cargo test -p loopflow --test integration documented_commands::` passed all four checks.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed.
- `scripts/test_desktop.sh --filter 'SettingsWorkflowTests|ConfigurationSettingsTests|ContextSettingsWorkflowTests|ThemeWorkflowTests|DesignDisciplineTests'` passed all five suites, including a cold start and a server restart with both scopes open. The checkout showed an offline file change and saved a Reset after reconnecting.
- QA round 4 ([qa.html](qa.html)) drove the branch app on a locked screen and verified Agent Policy fallbacks, emptied number field Reset, gated `In effect`, clean checkout `git status`, and broken-YAML live recovery.
- After simplifying declaration edits and moves, `scripts/test_desktop.sh --filter 'SettingsWorkflowTests|ConfigurationSettingsTests|ContextSettingsWorkflowTests'` passed all three suites. The compact-window diagnostic was skipped headless.
- QA round 5 ([qa.html](qa.html)) on a locked screen: both scopes reconnected by themselves 11–16 s after a private server restart, kept their last reading, dropped an edit typed while down, and saved edits afterwards.
- After unifying the settings writers, `cargo test -p loopflow-execution --lib -- first_edit_ hand_edit_during_write_is_preserved destinations_bind_save_explanation served_scope_sees` passed five tests. A first Wave edit detected both changed frontmatter and a newly created file with matching contents.
- `scripts/test_desktop.sh --skip-build` passed six focused checkpoint and reconnect tests plus `checkoutDiffFromSettingsToLaunch`, `scopedEditsAndPromoteRoundTripThroughTheCLI`, and `personalAndProjectSettingsRecoverAfterServerRestart` against the rebuilt CLI. Gate runs the suites.

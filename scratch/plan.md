# Settings redesign · LOO-1066

> Visual redesign of the settings pane. make it more like a standard Apple Settings pane, sort of like Rex

Direction on record: [demo-notes.md](demo-notes.md) (verbatim feedback from Jack Heart during the live demo of PR #2033), [design.md](design.md), [explore.html](explore.html), [qa.html](qa.html), and [concept-review.html](concept-review.html). User docs: [Surfaces](../docs/surfaces.md#settings), [Configuration](../docs/config.md#inspect-effective-settings), [Connect clients](../docs/server.md#phone-access), and [Loopflow for macOS](../swift/README.md).

## Settings rows

The personal Agents pane names each agent preference’s own fallback. Project &
Wave Config shows the destination path once and explains precedence when a
declaration participates. Choice controls and Undo use the setting’s display
names. Simple controls reset immediately, including an emptied number field.
YAML editors keep their reset preview.

The catalog’s applicability now determines both the sections available for a
Skill or Graph step and the scope a search result opens. Appearance shows its
configuration path directly. Identical search errors share one refresh action.

## Verification

- `cargo test -p loopflow-execution --lib config::` passed 113 tests, including the shared catalog fixture.
- `cargo test -p loopflow --test integration documented_commands::` passed all four checks.
- `scripts/test_desktop.sh --filter 'SettingsWorkflowTests|ConfigurationSettingsTests|ContextSettingsWorkflowTests|ThemeWorkflowTests|DesignDisciplineTests'` covered 24 headless tests. The checkout workflow observed a saved value before the preceding edit finished. It passed by name after waiting for the control to become editable. One compact-window diagnostic requires a display.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed.
- After simplification, `scripts/test_desktop.sh --skip-build --filter 'SettingsWorkflowTests|ConfigurationSettingsTests|ServerProtocolTests.*configuration'` passed 16 tests after a Desktop build. The compact-window check awaits a display. The built CLI returned the selected settings for checkout, Skill and global reads and answered a subscription checkpoint.
- QA round 4 ([qa.html](qa.html)) drove the branch app on a locked screen. Config locks moved from beside each file to `$LF_HOME/locks/config/`, so Settings writes leave no untracked `.lock` in the checkout. `config::` then passed 114 tests.
- At visual review, check sidebar chrome and the scoped form in a live Settings window. Settings keeps a dead subscription after a server restart until it is reopened.

## Already built on this branch (keep intact)

- **Policy B sidebar & window:** 6 personal panes (`General`, `Appearance`, `Shortcuts`, `Agents`, `Machines`, `Mobile`) + 1 `Project & Wave Config` pane, monochrome SF Symbols, 2 unselectable dividers, capsule search bar, independent `personal` / `project` live subscriptions, `Open ~/.lf/config.yaml` and `Open file…`.
- **Rust backend:** Bulk `config.scope` read/subscription (median 16.4 ms, p95 125.6 ms across 47 entries), `observe::invalidate_path` synchronous cache invalidation + `read_stream::wake_file_readers()`, `config.preview` / `config.edit` checked YAML writer preserving comments and 3-way absent/`null`/value semantics, `LandMethod` enum, `Definition.unset` wire field, `config.promote` atomic/resumable 2-file move + idempotent `Action::Dismiss`, `machine.status` concurrent SSH probes, and `lf config scope` CLI command.
- **Desktop surfaces:** `ScopedSettingsModel`, `SettingsUndo`, `AppearanceStore` instant-apply + Undo + staged `ThemeEditor`, `AutosaveToggle`, shared `KeyboardShortcutsView`, lifted `MobileSetup`, and `MachineConnections`.

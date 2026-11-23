# Settings redesign · LOO-1066

> Visual redesign of the settings pane. make it more like a standard Apple Settings pane, sort of like Rex

Direction on record: [demo-notes.md](demo-notes.md) (verbatim feedback from Jack Heart during the live demo of PR #2033), [design.md](design.md), [explore.html](explore.html), [qa.html](qa.html), and [concept-review.html](concept-review.html). User docs: [Surfaces](../docs/surfaces.md#settings), [Configuration](../docs/config.md#inspect-effective-settings), [Connect clients](../docs/server.md#phone-access), and [Loopflow for macOS](../swift/README.md).

## Remaining slice: align row concepts with Policy B

The Policy B shell (six personal panes + one `Project & Wave Config` pane, monochrome sidebar, two dividers, capsule search, independent personal/project subscriptions) and the Rust backend (`config.scope`, checked YAML writes, `observe::invalidate_path`, `config.promote` + `Action::Dismiss`, `machine.status`) are built and passing. Post-build concept review ([concept-review.html](concept-review.html)) found four row-level mismatches left over from the earlier raw-file editor. Finish them in one focused slice, ordered so the most visible bugs clear first:

### 1. Fix Agent Policy `unsetText` and null defaults (`catalog.rs`, `ScopedSettingRow.swift`)

- **Why:** `scope.settings["agent"]` holds the combined winning agent across `agent`, `default_agent`, and `agents`, while `ScopedSettingRow.unsetText` filters `effective.candidates` by `candidate.key == key` only for `default_agent` and `agents` (and bypasses `definition.unset`). On the personal `Agents` pane, an unset `Primary agent` (`agent`) currently renders `claude (from My global preferences)` when only `default_agent: claude` is set, and an unset `Starting preference` (`default_agent`) renders `None (Default)`.
- **Rust (`rust/loopflow-execution/src/config/catalog.rs` + `tests/fixtures/dto/configuration_catalog.json`):**
  - Add `unset` phrases to `agent` (`unset "Use starting preference"`) and `default_agent` (`unset "First in fallback order"`).
- **Swift (`swift/LoopflowMac/Views/ScopedSettingRow.swift`):**
  - Treat all three Agent Policy keys (`agent`, `default_agent`, `agents`) uniformly in `unsetText`: check `effective.candidates.last(where: { $0.key == key })`. If found, show `labeled(candidate.value.settingsText, origin: title(locator: candidate.locator))`. If absent, use `definition.unset` when present, else the decoded catalogue default (`Empty list (Default)` for `agents`).

### 2. Show `In effect` provenance only where a real layer participates (`ScopedSettingRow.swift`, `ScopedSettings.swift`)

- **Why:** `ScopedSettingRow` currently renders `SettingProvenance("In effect", …)` unconditionally on every non-global, non-policy row, stacking ~16 duplicate `› In effect · X · Default` disclosures directly underneath controls that already display `X (Default)`.
- **Swift (`ScopedSettingRow.swift`, `ScopedSettings.swift`):**
  - In `Project & Wave Config`, render `SettingProvenance("In effect", …)` only when `declared != .absent || effective.candidates.contains { $0.source != "builtin" }`.
  - Show the active file's path once in the bottom file section beside `Open file…` (secondary/mono text, selectable) so the destination path is visible even on a clean checkout with no overrides.

### 3. Clean Picker choices, hide no-op `Explicit null`, unify Reset + Undo (`ConfigurationScope.swift`, `ScopedSettingRow.swift`, `ScopedSettingsModel.swift`)

- **Why:** Rust's resolver skips YAML `null` identically to omitting the key (`settings.rs:526`), so offering `Explicit null` as a selectable menu item on every nullable Picker lets people author a no-op. Choice pickers also display raw lowercase schema tokens (`gh`, `local`, `default`, `priority`, `on`, `off`, `squash`, `merge`, `rebase`). Meanwhile, `Reset…` opens a 2-step raw YAML diff (`Reset in this file`) even on personal panes, contradicting instant-apply + Undo used by Picker `(Default)` and clearing a text field; Undo notices print raw keys (`Saved repo_root in My global preferences`); and clearing an integer field (`Automatic retries`) fails validation instead of resetting.
- **Swift:**
  - **No-op `Explicit null`:** Keep `ConfigurationScope.Authored.present(.null)` in the data model so existing hand-written `key: null` is recognized and resettable, but render `Text("Explicit null").tag(.present(.null))` inside a Picker **only when `declared == .present(.null)`**.
  - **Human choice labels:** Add `ConfigurationDefinition.displayText(for:)` (used by Pickers, `effectiveText`, `unsetText`, and `SettingProvenance`, while writes continue using `settingsYAML`):
    - `land`: `gh` → `"GitHub PR"`, `local` → `"Local merge"`
    - `codex.service_tier`: `default` → `"Standard"`, `priority` → `"Fast"`
    - `auto`: `on` → `"On"`, `off` → `"Off"`
    - `merge_method`: `squash` → `"Squash"`, `merge` → `"Merge"`, `rebase` → `"Rebase"`
  - **Consistent Reset:** For simple controls (`toggle`, `choices`, `text`, `integer`, `stringList`), clicking **Reset** calls `save(.reset, to: file)` immediately (still checked against `slot.entries`, with exact `ConfigurationEditReceipt` Undo). Keep the 2-step live YAML diff preview (`Reset…` → `Cancel` / `Reset in this file`) only for raw `.yaml` controls (`repo.tracker`, `machine`, `diff_groups`, `release`, `context.budgets`), matching their explicit `Apply` / `Cancel` model.
  - **Empty integer fields reset:** In `ScopedSettingRow.commitText()`, when `text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty` on `.integer`, emit `definition.change(to: nil, in: draft.file)` (which resets cleanly instead of failing validation with `""`).
  - **Human titles in Undo notices:** In `ScopedSettingsModel.apply` and `promote`, look up `catalogue.first(where: { $0.key == key })?.title ?? key` and phrase notices cleanly: `"Saved \(title)"` / `"Reset \(title)"` on `.personalGlobal`, and `"Saved \(title) in \(owner.title)"` / `"Reset \(title) in \(owner.title)"` / `"Moved \(title)"` on scoped files.

### 4. Concrete target naming, Launch-only filtering, and minor chrome cleanup (`SettingsTargetPicker.swift`, `ScopedSettings.swift`, `SettingsWindow.swift`, `ThemeChooser.swift`)

- **Why:** Three unrelated disclosures are labeled `Advanced` (Skill/Graph-step targeting in `SettingsTargetPicker`, secondary/inactive catalog keys in `ScopedSettings`, and `~/.lf/config.yaml` path in `ThemeChooser`). Selecting a Skill or Graph step leaves 10 non-Launch settings on screen as unexplained read-only rows, and jumping from Search to a non-Launch key doesn't clear `project.target`.
- **Swift:**
  - Rename `SettingsTargetPicker`'s `DisclosureGroup("Advanced")` to `DisclosureGroup("Skill or Graph step")`.
  - Pass `targeted: model.target != .general` into `SettingsSection.appears(in:wave:targeted:)` and filter section keys and `other` (`Advanced settings`) to `applicability == "launch"` while targeted (hiding empty sections and the Wave-only `Git & Release` redirect). Show one clear note beside `Inspect general work`: `"Showing launch settings for this target. Repository and general settings apply to all work."`
  - In `SettingsWindow.open(_ match:)`, when opening a `.project` setting with `definition.applicability != "launch"` and `project.target != .general`, reset `project.target = .general`.
  - In `ThemeChooser`, replace `DisclosureGroup("Advanced")` with a quiet file section showing `Open ~/.lf/config.yaml` (or `Configuration file` path) matching `General` / `Agents` / `Machines`.
  - In `SettingsWindow`, remove the dead sidebar-toggle toolbar button (`.toolbar(removing: .sidebarToggle)`) so the Settings window doesn't render an empty toolbar strip above the sidebar search box and detail form; in `searchResults`, avoid duplicating identical failure messages when both `personal` and `project` report the same server error.

### 5. Update docs & tests

- Update `docs/surfaces.md` (Settings section) to reflect instant `Reset` on simple controls, **Skill or Graph step** disclosure name, and conditional `In effect` provenance.
- Update `SettingsWorkflowTests` and `ContextSettingsWorkflowTests` to verify:
  - Unset `Primary agent` on personal `Agents` shows `Use starting preference (Default)` even when `default_agent: claude` is set in `~/.lf/config.yaml`, and unset `Starting preference` shows `First in fallback order (Default)`.
  - Clean checkout rows in `Project & Wave Config` do not render `In effect`, while overridden/inherited rows (including losing Shared vs My toggles) do.
  - Nullable Pickers omit `Explicit null` unless `declared == .present(.null)`, and show human labels (`GitHub PR`, `Standard`, `On`).
  - Simple-control `Reset` resets immediately with human-titled Undo notice, and clearing `Automatic retries` resets cleanly.
  - Targeting a Skill hides non-Launch sections and searching a non-Launch key clears `project.target`.
- Run focused checks:
  - `cargo test -p loopflow-execution --lib config::` and `cargo test -p loopflow --test integration documented_commands::` + `cargo clippy --all-targets -- -D warnings`
  - `scripts/test_desktop.sh --filter 'SettingsWorkflowTests|ConfigurationSettingsTests|ContextSettingsWorkflowTests|ThemeWorkflowTests|DesignDisciplineTests'`

## Already built on this branch (keep intact)

- **Policy B sidebar & window:** 6 personal panes (`General`, `Appearance`, `Shortcuts`, `Agents`, `Machines`, `Mobile`) + 1 `Project & Wave Config` pane, monochrome SF Symbols, 2 unselectable dividers, capsule search bar, independent `personal` / `project` live subscriptions, `Open ~/.lf/config.yaml` and `Open file…`.
- **Rust backend:** Bulk `config.scope` read/subscription (median 16.4 ms, p95 125.6 ms across 47 entries), `observe::invalidate_path` synchronous cache invalidation + `read_stream::wake_file_readers()`, `config.preview` / `config.edit` checked YAML writer preserving comments and 3-way absent/`null`/value semantics, `LandMethod` enum, `Definition.unset` wire field, `config.promote` atomic/resumable 2-file move + idempotent `Action::Dismiss`, `machine.status` concurrent SSH probes, and `lf config scope` CLI command.
- **Desktop surfaces:** `ScopedSettingsModel`, `SettingsUndo`, `AppearanceStore` instant-apply + Undo + staged `ThemeEditor`, `AutosaveToggle`, shared `KeyboardShortcutsView`, lifted `MobileSetup`, and `MachineConnections`.

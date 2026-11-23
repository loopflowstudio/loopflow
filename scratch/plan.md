# Settings redesign · LOO-1066

> Visual redesign of the settings pane. make it more like a standard Apple Settings pane, sort of like Rex

Jack Heart chose “vision c” at explore. [Vision C](design.md) and its [prototype](explore.html#vision-c) record the direction. The [concept review](concept-review.html) audits the built branch against its base and specifies the fixes below. [Choices for review](questions.md) records the defaults. User docs: [Surfaces](../docs/surfaces.md#settings), [Configuration](../docs/config.md#inspect-effective-settings), [Connect clients](../docs/server.md#phone-access), and [Loopflow for macOS](../swift/README.md).

## Done when

⌘, opens a native macOS Settings window with a searchable sidebar and grouped Apple Settings forms. App contains General, Appearance, Shortcuts and Mobile. Workflow contains Agents, Context and Git & Release. Infrastructure contains Machines. Use system chrome and accent with colored symbols. Theme cards show their own colors, and the chosen theme reaches Desktop, Ghostty and iPhone through the existing appearance path.

In the Workflow and Machines panes, **Applies to** names the exact work being inspected — Global, a checkout, a Wave, and any active Skill or Graph step — and **Save in** chooses the declaration audience (`My` vs `Shared`) at that scope. A setting’s own applicability never hijacks `Save in` into a ghost destination. Global-only Machine settings edit `~/.lf/config.yaml` directly in a clearly labeled **This Machine (Global)** section, and Git & Release explains clearly when a Wave is selected.

Each row edits the declaration in its file, preserving the three-way distinction between absent, explicit `null`, and an authored value. Coupled agent policy (`agent`, `default_agent`, `agents`) is grouped in one section with one shared effective result. Inactive compatibility keys and internal Desktop state stay out of the main forms (reachable through Advanced/search). Server writes invalidate the file observation cache synchronously so rapid edits never hit stale slots. A single-row validation or conflict error stays on that row without locking the rest of Settings; only a read failure marks the scope stale. Interrupted 2-file moves surface on the row and can be either resumed or explicitly dismissed if a file diverged. Search opens its pane, scrolls the form to the row and highlights it briefly.

## Built

Each item below exists on the branch. The [concept review](concept-review.html) sets out why.

- **Live configuration.** `observe::invalidate_path` (`rust/loopflow-execution/src/observe.rs`) runs from `config::files` atomic writes and from promotion journal writes. It refreshes the observed file, its metadata and the parent listing, then wakes subscriptions. Three checked writes in a row read back through `config.scope`.
- **Error boundaries.** `ScopedSettingsModel.errors` is keyed by `RowAddress`. A validation error or a stale-slot conflict stays on its row. Only a save in progress or a scope read failure disables edits.
- **Applies to / Save in.** The header shows the active Skill or step. `Save in` offers only My/Shared at the selected scope, and none at Global. Machines has *Connection health* (read from `config.scope`, keeps the last check while probing, Refresh button), *Placement* and *This Machine (Global)*, which writes `~/.lf/config.yaml` without changing `Save in`. At Wave scope, Git & Release shows one repository banner.
- **Forms.** Curated sections per pane (`ScopedSettings.swift`), including one Agent Policy section with a shared result. Inactive and internal keys live under Advanced and stay searchable. `ConfigurationScope.Authored` keeps absent, explicit null and value apart. Nullable fields accept an explicit null, which the resolver skips. `land` is a `LandMethod` enum in `catalog.rs`. `user.name` and `logs.linear_requests` have native controls.
- **Recoverable moves.** `promotion::Action::Dismiss` removes a diverged journal under the move locks and leaves both files untouched. The row shows Finish and Dismiss actions, and a refreshed preview asks for another review.
- **Search and Appearance.** `settingsForm(highlighted:)` puts the `ScrollViewReader` inside each pane's `Form` and yields once before scrolling. Rows carry `.id(key)`. ThemeChooser keeps its controls and errors inside Sections.
- **Earlier passes.** The split window, sidebar groups, `AutosaveToggle`, shared `KeyboardShortcutsView`, `MobileSetup`, `AppearanceStore` instant-apply with Undo, `ThemeEditor` drafts, `config.scope` subscription (median 16.4 ms, p95 125.6 ms across 47 entries), checked YAML patching, the locked 2-file move, the `machine.status` probe and legacy Wave `GOAL.md` adoption.
- **What the integration tests exposed.** YAML deserialization on its own accepts null as an empty map. A separate read after saving could show an intermediate value after Undo, because the subscription suppresses identical frames, so the editor confirms writes on its own subscription.

## Checks and remaining demo

- The Rust checked-edit, promotion, schema, shared DTO, nullable declaration, cache invalidation, file subscription, served configuration, and documented-command tests passed.
- The headless Desktop Settings, Context, Configuration, Theme, Mobile and Design Discipline suites passed against the branch server on a private Machine.
- `LOOPFLOW_NATIVE_TESTS=1 swift test --package-path swift --filter searchScrollsToTheLastContextRowInACompactWindow` passed. The mounted 520 × 240 form scrolled to the last Context row and back to the first group.
- Native window appearance checks passed in System, Light and Dark modes. Rust formatting and Clippy passed with warnings denied.
- `cargo test -p loopflow-execution --lib config::` passed 113 tests after isolating the private-file migration fixture from concurrent PATH changes.
- `LOOPFLOW_NATIVE_TESTS=1 swift test --package-path swift --filter 'SettingsWorkflowTests|ConfigurationSettingsTests|hiddenPatternsKeepWhereTheyCameFromAndOnlyYoursChange|themeReadIncludesTheSlotForACheckedEdit'` passed 17 tests against the rebuilt CLI. The launch check reads the README patch delivered to Pi.
- QA round 2 ([qa.html](qa.html)) drove live edits, row errors, explicit null, interrupted moves, This Machine (Global) and search through the branch server and an isolated Dev bundle. It fixed the setting error wording, a repeated Dismiss, the null label and exact-key search ranking.
- Gate owns hosted CI. The remaining visual review is the full split window in Light and Dark at different sizes. Run `uv run python scripts/loopflow-dev.py run` against the branch server on a private Machine for that demo.

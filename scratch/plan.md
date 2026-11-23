# Settings redesign · LOO-1066

> Visual redesign of the settings pane. make it more like a standard Apple Settings pane, sort of like Rex

Jack Heart chose “vision c” at explore. [Vision C](design.md) and its [prototype](explore.html#vision-c) record the direction. The [concept review](concept-review.html) audits the built branch against its base and specifies the fixes below. [Choices for review](questions.md) records the defaults. User docs: [Surfaces](../docs/surfaces.md#settings), [Configuration](../docs/config.md#inspect-effective-settings), [Connect clients](../docs/server.md#phone-access), and [Loopflow for macOS](../swift/README.md).

## Done when

⌘, opens a native macOS Settings window with a searchable sidebar and grouped Apple Settings forms. App contains General, Appearance, Shortcuts and Mobile. Workflow contains Agents, Context and Git & Release. Infrastructure contains Machines. Use system chrome and accent with colored symbols. Theme cards show their own colors, and the chosen theme reaches Desktop, Ghostty and iPhone through the existing appearance path.

In the Workflow and Machines panes, **Applies to** names the exact work being inspected — Global, a checkout, a Wave, and any active Skill or Graph step — and **Save in** chooses the declaration audience (`My` vs `Shared`) at that scope. A setting’s own applicability never hijacks `Save in` into a ghost destination. Global-only Machine settings edit `~/.lf/config.yaml` directly in a clearly labeled **This Machine (Global)** section, and Git & Release explains clearly when a Wave is selected.

Each row edits the declaration in its file, preserving the three-way distinction between absent, explicit `null`, and an authored value. Coupled agent policy (`agent`, `default_agent`, `agents`) is grouped in one section with one shared effective result. Inactive compatibility keys and internal Desktop state stay out of the main forms (reachable through Advanced/search). Server writes invalidate the file observation cache synchronously so rapid edits never hit stale slots. A single-row validation or conflict error stays on that row without locking the rest of Settings; only a read failure marks the scope stale. Interrupted 2-file moves surface on the row and can be either resumed or explicitly dismissed if a file diverged. Search opens its pane, scrolls the form to the row and highlights it briefly.

## Experience and model

### 1. Live file cache and error boundaries (Rust + Swift)

- **Invalidate `observe::Cache` on config writes.** Add `observe::invalidate_path(path: &Path)` on `Cache::shared()` to immediately re-observe matching `Fact::File`, `Fact::Metadata`, and parent `Fact::Directory` entries (advancing their revisions so active `read_stream` subscriptions push immediately without waiting for the 1-second poll). Call it from `config::files::atomic_write` and when creating or removing pending move journals in `engine::config::promotion`.
- **Keep read failures and row edit errors separate.** `ScopedSettingsModel.editsDisabled` disables writes only during an active save (`saving`) or a scope read failure (`reading.failure`, e.g., malformed YAML or unreachable server, where the last confirmed reading stays visible with a leading Refresh button). Validation errors and stale-slot conflicts belong to the row that triggered them:
  - A validation error (invalid graph name, bad YAML, invalid integer) renders inline on that row while keeping its draft editable and every other row across Settings interactive.
  - A stale-slot conflict renders inline on that row with a **Refresh** action, keeping the draft attached to its original slot until refreshed.

### 2. Honest `Applies to`, `Save in`, and pane applicability

- **`Applies to` shows the full inspected context.** Display repository display names (not raw full paths) in the picker and label, and include any active Skill or Graph step target (`<repo> · Skill <name>` or `<repo> · <graph> / <step>`) directly in the visible `Applies to` header alongside a one-click reset to general work so a collapsed `Advanced` disclosure never hides an active target.
- **`Save in` never holds ghost destinations.** At Global, hide `Save in`. For a checkout, offer only `My repository settings` and `Shared repository settings`. For a Wave, offer only `My Wave preferences` and `Shared Wave preferences` (falling back to Shared if the Wave has no authored ID). Remove the `owner == model.destination` escape hatch.
- **Pane applicability matches each setting’s owner:**
  - **General** is always Global (`PersonalGlobal`) with no `Applies to` / `Save in` picker and no multi-layer provenance disclosure on every row.
  - **Machines** separates three clean sections:
    1. **Connection health** — reads the `machines` list from `config.scope`’s `PersonalGlobal` slot (deleting the redundant `config.get` poll), preserves last-known per-Machine health across 30-second background refreshes instead of wiping `health = [:]` back to “Checking…”, and includes an explicit Refresh button.
    2. **Placement** (`machine`) — the one scoped Machine preference (`PersonalScoped`: Global, My repo, or My Wave). When `Save in` is Shared, state plainly that Machine placement is personal-only and offer `Switch Save in to My …` (switching between valid options of the current scope).
    3. **This Machine (Global)** (`hub`, `machines`, `connections`, `pools`, `ondemands`, `forward.env`, `forward.files`) — edits `PersonalGlobal` (`~/.lf/config.yaml`) directly without mutating `model.destination`, clearly labeled so `Save in` and sidebar override dots stay intact across panes.
  - **Git & Release** at Wave scope: because all seven keys have `Applicability::Repository` (`scope != Wave`), show one clear section banner (*“Git & Release settings apply to the repository, not individual Waves.”*) with a single action to switch `Applies to` to the checkout, instead of 21 per-row `Edit in …` buttons.
  - **Unnamed Graph steps:** when inspecting a step with no `id:`, show one banner explaining that unnamed steps are read-only until given an `id:` in their Graph YAML, and keep each row’s provenance `DisclosureGroup` accessible so candidates remain inspectable.

### 3. Grouped Apple Settings forms and faithful `Authored` controls

- **Curated sections backed by `config.catalog`.** Replace the flat one-`Section`-per-key loop with semantic grouped `Section`s per pane (matching Apple System Settings):
  - **General:** *Task Files* (Autosave), *Profile & Workspace* (`user.name` as a native TextField, `repo_root`), *Updates & Diagnostics* (`stream`, `logs.linear_requests` as a native Toggle, `session`).
  - **Agents:** *Agent Policy* (`agent`, `default_agent`, `agents` together in one section with one shared **Effective agent order** result and provenance ladder that explains when a Pin overrides Starting preference / Fallback order), *Execution* (`yolo`, `wave.graph`, `auto`, `automation`, `supported_harnesses`), *Harnesses* (`claude.model`, `codex.service_tier`, `chrome`).
  - **Context:** *Diff & Clipboard* (`context.diff`, `context.diff_files`, `context.paste`), *Files & Budgets* (`context.docs`, `context.exclude`, `context.budgets`).
  - **Git & Release:** *Git & Pull Requests* (`land`, `merge_method`, `pr`), *Diff View* (`diff_hidden`, `diff_groups`), *Planning & Release* (`repo.tracker`, `release`).
  - **Machines:** *Connection health*, *Placement*, *This Machine (Global)*.
  - **Hide inactive and internal keys from primary forms.** Keep `Lifecycle::Inactive` keys (`isolate`, `summaries`, `summary_tokens`), internal Desktop state (`portfolio`), and `Category::Accounts` (`auth_browsers`, no longer falling into `Agents`) out of the primary forms. Expose any authored inactive/other catalog keys in a collapsed **Advanced** section (and keep them searchable with an `Inactive` tag for inactive entries).
  - **Compact rows when clean.** On **General** (single file), omit provenance disclosures entirely and show a quiet Reset button only when declared. On **Global** in scoped panes, keep rows compact without duplicating every value in a second line; show provenance on demand or when inspecting a checkout/Wave where layers interact.
- **Preserve `Authored::{Absent, Present(Value)}` end-to-end in Swift.** Decode `ConfigurationScope.Slot.declared` into a typed `enum Authored { case absent; case present(JSONValue) }` instead of collapsing into `JSONValue?`:
  - **`Option<bool>` (`chrome`):** render a 3-way `Picker` (`Inherited`, `Claude default (null)`, `On`, `Off`) rather than a 2-state `Toggle` that conflates `null` with `false`.
  - **`Option<Enum>` (`codex.service_tier`, `auto`, `merge_method`):** use a 3-case selection tag (`inherited`, `clear` for explicit `null`, `value(JSONValue)`) so explicit `null` is never mislabeled `"Inherited"`.
  - **`land`:** type `land` as a 2-variant enum (`gh` | `local`) in `catalog.rs` (or map its two choices to a `Picker` in Swift; typing it in Rust keeps CLI and catalog validation aligned).
  - **`Option<String>` (`agent`, `default_agent`, `claude.model`, `wave.graph`, `repo_root`, `stream`):** never convert `.present(.null)` to literal `"None"` in a `TextField`; show an explicit cleared state, and treat clearing a text field to empty as `.reset` (removing the declaration) rather than writing `""`.
  - **Ordered lists (`OrderedSetting`):** show a concise summary on the collapsed header, and show `"Save empty list"` only when `definition.combination == "append_reset"` and the destination can inherit from a broader layer.
  - **Simple General maps (`user`, `logs`):** expose `user.name` and `logs.linear_requests` as native controls (preserving any sibling fields on edit) instead of raw YAML disclosures.

### 4. Recoverable 2-file moves (`promotion.rs` + `ScopedSettingRow.swift`)

- **Add `Action::Dismiss` to `engine::config::promotion`.** When an interrupted 2-file move’s journal exists (`~/.lf/settings-moves/<hash>.json`), `Action::Resume` continues to require matching states before installing. If a file was hand-edited and `Resume` refuses, `Action::Dismiss` acquires the ordered file locks, deletes the journal without modifying either config file, invalidates the journal directory in `observe::Cache`, and returns `Outcome::Dismissed`. Update the wire schema and DTO fixtures (`tests/fixtures/dto/server_schema.json`, `configuration_scope.json` if touched) in the same change.
- **Surface pending moves and preview changes clearly in Swift.** Show a pending move banner/actions directly on the row (not buried inside a collapsed disclosure) with **Finish interrupted move…** and, on divergence, **Dismiss pending move**. In a move preview, name the audience that gains the declaration (*“Everyone using this repository”* for Shared repo, *“All work in this checkout”* for My repo). When `Confirm move` returns a refreshed `Outcome::Preview` because the effective value changed materially, show an explicit inline notice asking the person to review the updated preview before confirming again.

### 5. Search scroll and Appearance form polish

- **Fix Search → Scroll to Row.** Place `ScrollViewReader` *inside* `ScopedSettings`, `ThemeChooser`, and `MobileSettings` around their `Form` (not outside the pane switch in `SettingsWindow`). Remove the conflicting `.id(RowAddress(...))` wrapper around `ScopedSettingRow`’s `Section` (bind per-row draft state to destination/key changes via `.onChange` so `.id(definition.key)` stays the sole scroll identity inside `Form`). When switching panes from search, yield one layout turn (`await Task.yield()`) before calling `proxy.scrollTo(key, anchor: .center)`.
- **Group `ThemeChooser` controls in `Section`s.** Wrap the Appearance mode picker and Customize button in a `Section`, and move `appearance.readFailure` / `appearance.notice` into a leading error `Section` at the top of the `Form` (matching `ScopedSettings`) so errors never scroll off-screen.

## Build order

Start with the slice that proves the core model fixes fastest, then layer the UI structure and recovery:

1. **Slice 1 — Rust cache invalidation, `Action::Dismiss`, and `land` enum:**
   - `observe::invalidate_path` called from `config::files::atomic_write` and `promotion.rs`.
   - `promotion::Action::Dismiss` + `Outcome::Dismissed` + Rust unit test covering diverged interrupted move → `Resume` fails → `Dismiss` clears journal while keeping both files.
   - Type `land` as an enum (`gh` | `local`) in `catalog.rs` (keeping `serde` string compatibility) and update DTO/server schema fixtures (`tests/fixtures/dto/server_schema.json`).
   - Verify with focused Rust tests (`promotion::tests`, `config::files::edit::tests`, `served_reads`, `the_checked_in_schema_is_what_describe_prints`) and confirm rapid back-to-back writes reflect immediately in `config.scope`.
2. **Slice 2 — Swift `Authored` model, row-scoped errors, honest `Save in` / `Applies to`, and `MachineConnections`:**
   - Replace `Slot.value: JSONValue?` with `enum Authored { case absent; case present(JSONValue) }`.
   - Remove `editError` from `ScopedSettingsModel.editsDisabled`; move validation/conflict errors to per-row state.
   - Show active `model.target` and repository display names in `Applies to`; remove the ghost `Save in` fallback; give `Machines` its **This Machine (Global)** section and `Git & Release` its Wave-scope banner; add provenance + read-only explanation for unnamed Graph steps.
   - Drive `MachineConnections` from `config.scope`’s `PersonalGlobal` `machines` slot and preserve previous health states across 30s refreshes.
3. **Slice 3 — Grouped Apple Settings forms, faithful nullable controls, Agent Policy section, Search scroll, and `ThemeChooser` sections:**
   - Group rows into semantic `Section`s per pane; hide `Lifecycle::Inactive` (`isolate`, `summaries`, `summary_tokens`), `portfolio`, and move `auth_browsers` out of `Agents` into Advanced/collapsed catalog access.
   - Implement 3-state `chrome` picker, 3-state nullable enum pickers, clean nullable string TextFields, native `user.name` / `logs.linear_requests` controls, contextual `Save empty list`, and the grouped **Agent Policy** section.
   - Surface pending moves + **Dismiss pending move**, audience text, and refreshed-preview notice in `ScopedSettingRow`.
   - Fix `ScrollViewReader` inside `Form` and group `ThemeChooser` sections with leading errors.
   - Update tests (`SettingsWorkflowTests`, `ContextSettingsWorkflowTests`, `ConfigurationSettingsTests`, `ThemeWorkflowTests`, `DesignDisciplineTests`) to exercise every fixed interaction, including rapid toggles, row-scoped typo recovery, Machines Global edit without hijacking `Save in`, diverged move dismiss, nullable controls, and Search scroll.

## Built (from earlier passes)

The split window, sidebar groups, `AutosaveToggle`, shared `KeyboardShortcutsView`, lifted `MobileSetup`, `AppearanceStore` instant-apply + Undo + `ThemeEditor` drafts, `config.scope` subscription (median 16.4 ms, p95 125.6 ms across 47 entries), `config::files::edit` checked YAML patching, `promotion` locked 2-file move, `machine.status` served identity/rental probe, legacy Wave `GOAL.md` adoption, and sidebar search matching are built and passing their existing tests.

## Gate and demo

Run `scripts/test_desktop.sh --filter 'SettingsWorkflowTests|ContextSettingsWorkflowTests|ConfigurationSettingsTests|ThemeWorkflowTests|MobilePairingTests|DesignDisciplineTests'` and focused Rust checks (`promotion`, `edit`, `served_reads`, `server_schema`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`). Run the iOS protocol check if `ServerProtocol.swift` changes.

For the visual demo, run `uv run python scripts/loopflow-dev.py run` against the branch server on a private Machine. Verify Search actually scrolls to bottom rows (`context.budgets`, `context.paste`), verify `Machines` edits Global settings without mutating `Save in`, verify a typo error keeps the field editable and other panes unlocked, verify Agent Policy grouping and 3-state `chrome`, and check Light and Dark appearance with resizing.

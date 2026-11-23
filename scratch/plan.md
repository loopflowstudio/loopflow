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
  - **Null on disk.** The existing resolver skips null declarations, and the setter used to reject them even for nullable types. Accept explicit null for nullable declarations, label it **Explicit null**, and show the resolver’s effective value separately. Reset still removes the key.
  - **`Option<bool>` (`chrome`):** render a 3-way `Picker` (`Inherited`, `Explicit null`, `On`, `Off`) rather than a 2-state `Toggle` that conflates `null` with `false`.
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

## Implementation

- **Live configuration.** Atomic writes and removal of move journals refresh observed files, metadata and parent-directory listings, then wake file subscriptions. `config.promote` accepts `dismiss` under the move locks. The catalog supplies the two landing methods. Three successive checked writes are covered through `config.scope`.
- **Scope and destination.** Each file carries typed absent/present declarations. A rejected write stays on its addressed row. The header shows the active Skill or step. Machine health reads the global declaration from the scope and keeps each identity’s last check while probing. Global Machine edits leave the chosen audience intact.
- **Forms and recovery.** Grouped sections share one Agent Policy result. Nullable pickers, native General fields and ordered lists edit the selected file. Search reveals the row inside its form. Pending moves expose recovery actions and changed confirmations require another review.
- **What the integration tests exposed.** YAML deserialization alone accepts null as an empty map. Only nullable fields can save an explicit null. The resolver continues to skip that declaration. A separate read after saving could leave Desktop showing an intermediate value after Undo because the subscription suppresses identical frames. The editor now confirms writes on its existing subscription.

## Built (from earlier passes)

The split window, sidebar groups, `AutosaveToggle`, shared `KeyboardShortcutsView`, lifted `MobileSetup`, `AppearanceStore` instant-apply + Undo + `ThemeEditor` drafts, `config.scope` subscription (median 16.4 ms, p95 125.6 ms across 47 entries), `config::files::edit` checked YAML patching, `promotion` locked 2-file move, `machine.status` served identity/rental probe, legacy Wave `GOAL.md` adoption, and sidebar search matching are built and passing their existing tests.

## Checks and remaining demo

- The Rust checked-edit, promotion, schema, shared DTO, nullable declaration, cache invalidation, file subscription, served configuration, and documented-command tests passed.
- The headless Desktop Settings, Context, Configuration, Theme, Mobile and Design Discipline suites passed against the branch server on a private Machine.
- `LOOPFLOW_NATIVE_TESTS=1 swift test --package-path swift --filter searchScrollsToTheLastContextRowInACompactWindow` passed. The mounted 520 × 240 form scrolled to the last Context row and back to the first group.
- Native window appearance checks passed in System, Light and Dark modes. Rust formatting and Clippy passed with warnings denied.
- Gate owns hosted CI. The remaining visual review is the full split window in Light and Dark at different sizes. Run `uv run python scripts/loopflow-dev.py run` against the branch server on a private Machine for that demo.

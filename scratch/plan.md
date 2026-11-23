# Settings redesign · LOO-1066

> Visual redesign of the settings pane. make it more like a standard Apple Settings pane, sort of like Rex

Direction on record: [demo-notes.md](demo-notes.md) (verbatim feedback from Jack Heart during the live demo of PR #2033), [design.md](design.md), [explore.html](explore.html), and [concept-review.html](concept-review.html). User docs: [Surfaces](../docs/surfaces.md#settings), [Configuration](../docs/config.md#inspect-effective-settings), [Connect clients](../docs/server.md#phone-access), and [Loopflow for macOS](../swift/README.md).

## Policy B & visual polish

During the live demo of PR #2033, Jack chose **Policy B** (`1b`, `2a`) plus a visual polish pass over the sidebar and controls:

**Built in this slice.** The seven-pane sidebar has two dividers, monochrome symbols, and a capsule search field. Personal preferences and Project & Wave Config have independent live readings. The project pane holds Context, Git & Release, agent overrides, and Machine placement. Search can open either an agent default or its scoped override. File-only global results reveal the global file button.

**Built: no "Inherited" display values.** `ScopedSettingRow.unsetText` labels an unset control with the value in effect and its origin (`On (Default)`, `GitHub PR (from Shared repository settings)`); the choice Picker's absent option, the TextField prompt and the ordered-list label all use it, and the provenance disclosure is always titled "In effect". `agents` and `default_agent` read the highest other declaration of their own key from the candidates, else the catalog default, because their effective value is the combined agent order. Absent, `Explicit null` and authored values stay distinct tags. `sharedCheckoutKeepsItsDestinationThroughValidationAndGlobalMachineEdits` fails if "Inherited" returns to any Settings pane.

**Built: null defaults name what decides.** A catalogue row may carry `unset` (wire field, nullable): `~/src`, `stable`, `Git's user.name`, `Off`, `Claude's own setting`, `Codex's own setting`, `This Machine`, `Squash when GitHub allows it`. Rows show it instead of "None (Default)". Live Light and Dark QA at compact and wide sizes is done ([qa.html](qa.html), round 3).

**Checks for this slice**

- `scripts/test_desktop.sh --skip-build --filter 'SettingsWorkflowTests|ConfigurationSettingsTests|ThemeWorkflowTests'` after building Desktop and `lf` — 20 passed, one compact-window scrolling check skipped headless.
- `loopflow-execution` libtest with `config::files:: config::keys:: engine::config::promotion::` — 24 passed.
- `cargo test -p loopflow --test integration documented_commands::` — 4 passed.
- Live Light and Dark windows at compact and wide sizes — checked in QA round 3 (screenshots in `scratch/qa-r3/`).

### 1. Clear policy for what belongs in `⌘,` Settings (Policy B · `2a`)

Separate calm **personal/app preferences** from **scoped repo/Wave `config.yaml` overrides** (following Rex, T3 Code, Rune, Zed, and Cursor):

- **Main sidebar panes are personal/global only** (`~/.lf/config.yaml` + Desktop prefs) — **no `Applies to`**, **no `Save in`**, **no `"Inherited"`**, **no per-row `Source` disclosures**, and **no raw YAML map textareas**:
  1. **General** — Display name (`user.name`), Autosave Task Files, Repositories folder (`repo_root`), Release channel (`stream`), Log Linear requests (`logs.linear_requests`), plus an **"Open `~/.lf/config.yaml`"** button.
  2. **Appearance** — Appearance mode (`System` / `Light` / `Dark`), Theme cards (instant-apply + `⌘Z` Undo), Customize (`ThemeEditor` keeps Apply/Cancel).
  3. **Shortcuts** — Grouped read-only keyboard shortcuts (shared with standalone Keyboard Shortcuts window).
  4. **Agents** — Personal defaults in `~/.lf/config.yaml`: Primary agent (`agent`), Starting preference (`default_agent`), Fallback order (`agents`), Run without asking (`yolo`), Claude model (`claude.model`), Codex service tier (`codex.service_tier`), Claude Chrome integration (`chrome`), Default Graph (`wave.graph`), Auto-ship new Tasks (`auto`), Automatic retries (`automation.retries`). Every control shows its **actual active or built-in default value** (never `"Inherited"`). Show a quiet `Reset` button only when the setting is explicitly set in `~/.lf/config.yaml`.
  5. **Machines** — Live **Connection health** (up to 4 concurrent SSH probes, Refresh), **Plan hub** (`hub`), plus **"Open `~/.lf/config.yaml`"** (and collapsed Advanced disclosure if needed) for map keys (`pools`, `ondemands`, `machines`, `connections`, `forward.*`) instead of stacking raw YAML textboxes on the main pane.
  6. **Mobile** — Tailscale switch + live green sidebar readiness dot + QR pairing + Copy/Share/Refresh.
- **One dedicated `Project & Wave Config` pane** (at the bottom of the sidebar, separated by a quiet divider) owns scoped repository/Wave/Skill/Graph-step overrides — including **Context** (`context.*`) and **Git & Release** (`land`, `merge_method`, `pr`, `diff_hidden`, `diff_groups`, `release`, `repo.tracker`), plus scoped **Agents** and **Machine placement** (`machine`):
  - Reuses the existing Rust `config.scope`, `config.preview`, `config.edit`, `config.promote`, `observe::invalidate_path`, Undo, and crash-journal Dismiss infrastructure intact.
  - Defaults **Applies to** to the active checkout (falling back to the first portfolio checkout or a Choose Checkout prompt when none is open), with **Save in** (`My` vs. `Shared`), **Advanced** Skill/Graph-step targeting, and an **"Open file…"** button for the selected YAML file.
  - **No `"Inherited"` as a control's displayed value anywhere.** Every picker/toggle displays the **actual effective value** (for example, `Standard (Default)` or `GitHub PR (from Global)` or `Off`), while still preserving absent / `Explicit null` / authored value distinctions in the file writer. Show a subtle override indicator + `Reset` / expandable `In effect` precedence ladder + `Move to…` only where meaningful (when overridden, losing to another layer, or expanded by the person) instead of stacking `"Inherited · …"` disclosures under every unset row.
  - Keep complex map/inactive keys (`diff_groups`, `release`, `context.budgets`, `isolate`, `summaries`, `summary_tokens`, `portfolio`, `auth_browsers`) out of the primary form under a collapsed **Advanced** section + **"Open file…"** (still searchable).

### 2. Visual polish pass (`1b` + demo feedback)

- **Quiet dividers instead of sidebar section text headers (`1b`):** Remove the `"App"`, `"Workflow"`, `"Infrastructure"` text headers in the sidebar. Group panes cleanly with native dividers:
  - `General`, `Appearance`, `Shortcuts`
  - *(divider)*
  - `Agents`, `Machines`, `Mobile`
  - *(divider)*
  - `Project & Wave Config` (shows subtle override dot when the inspected checkout/Wave file has overrides)
- **Minimalist monochrome SF Symbols:** Delete the rainbow colored square badges (`pane.badge.color` rounded rects). Use crisp, minimalist monochrome SF Symbols that adapt naturally to sidebar selection and Light/Dark mode.
- **More rounded sidebar search box:** Replace the 4 pt `CornerRadius.control` search box with a pill/capsule shape (`Capsule()` with subtle fill/border and clean padding) that feels native to macOS Settings.
- **Hand-named macOS Settings labels:** Give every visible setting a natural macOS Settings title (no raw key-derived `"Repo Root"`, `"Linear Requests"`, `"Codex · Service Tier"`, `"Supported Harnesses"`).
- **Clean spacing & contrast:** Tighten visual rhythm in Light and Dark modes across window sizes while keeping every explicit measure, font, and color compliant with `DesignDisciplineTests`.

### 3. Keep docs, search, and tests aligned

- Update cross-pane search (`SettingsSearch.swift`) so searching any catalog key routes cleanly to its Policy B home (`General`, `Agents`, `Machines`, `Appearance`, or `Project & Wave Config` for `Context` / `Git & Release` / scoped keys) and highlights the row.
- Update `docs/surfaces.md`, `swift/README.md`, and `scratch/questions.md` to reflect Policy B (6 personal panes + `Project & Wave Config`, monochrome sidebar with dividers, no `"Inherited"` display values).
- Update `SettingsWorkflowTests`, `ContextSettingsWorkflowTests`, `DesignDisciplineTests`, and `documented_commands` to cover the new Policy B navigation, real-value default display (no `"Inherited"` on Global pickers), monochrome sidebar/divider layout, and `"Open file…"` actions.

## Already built on this branch (keep intact)

- Rust bulk `config.scope` read/subscription (median 16.4 ms, p95 125.6 ms across 47 entries), `observe::invalidate_path` synchronous cache invalidation, `config.preview` / `config.edit` checked YAML writer preserving comments and 3-way absent/`null`/value semantics, `LandMethod` enum, `config.promote` atomic/resumable 2-file move + idempotent `Action::Dismiss`, `machine.status` concurrent SSH probes, and `lf config scope` CLI command.
- Swift `ScopedSettingsModel`, `SettingsUndo`, `AppearanceStore` instant-apply + Undo + staged `ThemeEditor`, `AutosaveToggle`, shared `KeyboardShortcutsView`, lifted `MobileSetup`, and `MachineConnections`.
- All Rust tests (`config::` 113, `promotion::` 2, `served_reads`, `documented_commands`, clippy `-D warnings`) and DTO fixtures (`tests/fixtures/dto/`).

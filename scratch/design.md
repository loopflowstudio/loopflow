# Settings Pane Visual Redesign (LOO-1066)

Reference prototype: [explore.html](explore.html) (Vision C).

## Direction on record

- Task brief: "Visual redesign of the settings pane. make it more like a standard Apple Settings pane, sort of like Rex"
- Jack Heart (at `explore`): "vision c"

## Vision C: Apple Settings + Live Override Lens

Replace the 5-tab top-toolbar `TabView` in `LoopflowApp.swift` (`Appearance`, `Editing`, `Configuration`, `Keyboard Shortcuts`, `Mobile`) with a standard macOS Settings window modeled on Apple System Settings and Rex: a `NavigationSplitView` sidebar with search and grouped inset `Form` detail panes.

### Sidebar & Panes

Grouped into three sections in the left sidebar (with a top search field):

1. **App**
   - **General** — folds in the 1-checkbox Editing pane (`Autosave Task files`), plus global `Loopflow` / `Desktop` preferences (`user`, `repo_root`, `stream`, `logs`).
   - **Appearance** — Appearance mode segmented control (`System` / `Light` / `Dark`), Theme card grid (built-in + yours + import tile), instant-apply on selection with Undo (`⌘Z` / Undo toast) instead of a modal Cancel/Apply footer; `Customize…` opens `ThemeEditor` (which keeps explicit Save/Apply and Cancel for multi-color edits).
   - **Shortcuts** — grouped read-only keyboard shortcut reference (Navigation, Task Files, App).
   - **Mobile** — Tailscale mobile access switch, live connection status dot in sidebar and detail row, QR code pairing card, Copy/Share address, Refresh.
2. **Workflow** (scope-aware)
   - **Agents** — `agent`, `default_agent`, `agents`, `yolo`, `claude.model`, `codex.service_tier`, `chrome`, `wave.graph`, `auto`, `automation.retries`, `supported_harnesses`.
   - **Context** — `context.diff`, `context.diff_files`, `context.paste`, `context.docs`, `context.exclude`, `context.budgets`.
   - **Git & Release** — `land`, `merge_method`, `pr`, `diff_hidden`, `diff_groups`, `release`, `repo.tracker`.
3. **Infrastructure** (scope-aware)
   - **Machines** — live reachability/status indicators, `hub`, `machine`, `machines`, `connections`, `forward.env`, `forward.files`, plus inline YAML disclosures for complex map keys (`pools`, `ondemands`).

### Key Mechanisms

1. **Typed Native Controls from `lf config catalog`**
   - Render catalog keys as native macOS controls inside `.formStyle(.grouped)` sections: `Toggle` for booleans, `Picker` / segmented `Picker` for enums and known choices, text/token controls for strings and string arrays, and a collapsible YAML editor disclosure row for complex object/map keys (`pools`, `ondemands`, `diff_groups`, `release`, `context.budgets`) or any future schema shape without a specialized renderer.
   - Replace manual path typing for repositories and waves with pickers populated from `PortfolioService` (and wave listing), keeping a fallback custom path option if needed.
   - Load all effective values and candidate layers for a scope in one Rust round-trip (wire `config.settings` or add a bulk `config.layer` read in `ServerProtocol.ConfigOperation` with DTO round-trip fixtures under `tests/fixtures/dto/`) rather than 47 sequential `config.get` calls.
2. **Scope Selector & Interactive Precedence Ladder**
   - Workflow and Infrastructure panes show a quiet scope selector in the header (`Global`, `<repo> · Personal`, `<repo> · Shared`, `<repo> / <wave> · Personal/Shared`).
   - On Global, rows stay uncluttered. When inspecting a narrower scope (or when a setting is overridden), rows show a subtle clickable provenance pill (e.g. `Inherited · Global` or `Personal repo` with an accent dot).
   - Clicking a row's provenance pill expands an inline precedence ladder showing every layer (`Default → Global → Shared repo → Wave → Personal repo`), which file won, a live YAML diff preview of the target file, and one-click actions: **Reset layer** (`config.unset`) and **Promote** (move a value up to a broader layer, e.g., Personal repo → Shared repo, with an Undo toast).
3. **Control-Level Sidebar Search**
   - Typing in the sidebar search box matches both pane names and individual settings across App settings and the `lf config catalog` (by label, key, and description).
   - Matching boolean settings can be toggled directly in the search list; clicking any search result jumps to its pane and briefly highlights the row.
4. **Live Sidebar Health Dots**
   - Mobile shows a live green dot when Tailscale mobile access is ready; scoped panes show a quiet override dot when the active non-Global scope has local overrides.

### Visual & Codebase Constraints

- **Apple Settings look + `DesignDisciplineTests`**: Keep the window feeling like native macOS System Settings / Rex (sidebar + inset grouped `Form` cards, crisp SF Symbols, native switches/pickers, light and dark mode). Every explicit font, spacing, padding, and corner radius in Swift must still pass `DesignDisciplineTests` (use `Typography.*`, `Spacing.*`, `CornerRadius.*`, `EmptyState`; never `.system(`, `.font(.caption)`, raw numeric padding/spacing/radius, or `ContentUnavailableView`).
- **DTO rules**: Any new or changed wire types between Rust and Swift get no `#[serde(default)]`, no Swift init defaults, and round-trip fixtures in `tests/fixtures/dto/`.
- **Delete what is replaced**: Remove the old 5-tab `TabView` shell and standalone single-key `ConfigurationSettingsView` once replaced (keep any reusable config destination / catalog model types that tests or the new views use).

### Open Threads (not yet explicitly specified by Jack)

Questions 2–4 from [explore.html](explore.html) were not called out separately when picking "vision c". Defaulting to Vision C's baseline unless steered otherwise:
- **Window skin (Q2):** Native Apple chrome (macOS System Settings surfaces, system accent, rounded-square colored SF Symbol badges in the sidebar — matches "standard Apple Settings pane, sort of like Rex"), tested in both Light and Dark modes and compatible with Loopflow themes.
- **Apply model (Q3):** Instant apply on click/toggle with Undo toast for simple controls and theme selection; explicit Apply/Cancel retained inside `ThemeEditor` and raw YAML disclosures.
- **Sidebar grouping (Q4):** Grouped sidebar (App · Workflow · Infrastructure) keeping Keyboard Shortcuts accessible in Settings (and its standalone `Keyboard Shortcuts` window).

# LOO-1067 Research Notes

## Exact input

- Jack Heart (2026-11-22): "lets also research what t3code, paseo, orchestrator inc, rune, cursor etc do to make nice agent-native file browsing experiences. and lets see what makes sense to move from the diff experience eg the comments, the sidebar expereince. lets also look at what were trying to do with teh auto scratch seelction and figure out alternatives to that because its not working well right now, leads to a lot of random flashing. also the nav/tabs is super awkward."
- Jack Heart (2026-11-22): "lets also look at integrating tools that command line geeks like like fzf / or jump to definitio / i think i want something between a and b, not any of the other layous, but show me another round of exploring leanign with that more basic experience but showing how we could lean into different directions based on other expereinces elsewehre / i also noticed taht cmd f seems to not be captured by the files browser and always goes to the main session"
- Jack Heart (2026-11-22): "lets do directions 1 and 2"
- Jack Heart (2026-11-22): "4a i think --read only by defautl / easy to turn on editable"
- Jack Heart (2026-11-22): "lets look at rex as an example here. maybe more of an api for opening a file in the pane and considering letting agents choose when to do it/ any thing to learn from paseo/t3/orchestrator/paradev etc on this /"

## Rex / explicit presentation API findings (Q3)

- **What Loopflow already has:** `lf open --file <path>` (`rust/loopflow/src/lf/commands/open/files.rs`) already resolves local and remote checkout paths and calls Desktop's `files` action (`DesktopActions.swift:514` -> `TaskFilesStore.presentFile`), quietly no-oping when no Desktop Files view is attached.
- **Two gaps today:**
  1. `write-docs.md` already uses `lf open --host desktop --file <path>`. The page-producing explore, concept-review, research and demo skills lack that presentation step.
  2. `TaskFilesStore.presentFile` routes through the exact same `follow()` heuristic as FSEvents (`TaskFileDocument.swift:448`), conflating passive disk writes with intentional presentation.
- **Rex (`superlogical/rex`):** Never guesses from disk writes. Gives agents an explicit CLI/server API (`rex split`, `rex run`, `rex focus`) plus `OSC 7501` status signals and teaches agents when to call them via `rex skill setup`.
- **Paseo / Paradev / T3 Code / Orchestrator Inc:** Daemon-backed explicit actions (`paseo`, `superset`, `ao preview`/`ao review`); passive writes only update change lists or badge background tabs — they do not yank focus out from under active reading.

## Root cause of `⌘F` going to the main session

Traced in `swift/LoopflowMac/`:
1. `WorkspacePanesView.swift:802-830`: `.graphLog` attaches `.background(PaneFocusTarget(isFocused: isFocused))` (which holds AppKit `firstResponder` so clearing terminal focus does not bounce back into `GhosttyTerminalNSView` via the key-view loop), whereas `.files, .changes` (`TaskFilesView`) does **not** attach `PaneFocusTarget`.
2. `TaskFilesView.swift` has no `⌘F` handler or find bar of its own. Only `TaskFileDocument.editor` (`NSTextView`, `usesFindBar = true`) handles `⌘F`, and only when that `NSTextView` is `firstResponder`. In rendered Markdown/HTML, `Diff`, or unfocused `Files`, `GhosttyTerminalNSView.performKeyEquivalent` (`GhosttyTerminalView.swift:1233`) stays `firstResponder` and opens the terminal's search bar instead.

## Existing codebase leverage for CLI-geek tooling

- **28 prebuilt Tree-sitter grammars already ship in the app** (`swift/LoopflowMac/Grammar.swift`, `TreeSitterGrammars.xcframework`), making fast zero-LSP symbol outlines and heuristic jump-to-definition (`tags.scm` + repo symbol index) practical across fresh Task worktrees without heavy LSP startup.
- **`lf task file <issue> <path> --at <rev>` already exists in Rust** (`rust/loopflow/src/lf/commands/read.rs:159`), and `TaskChangesSnapshot.commits` already supplies the branch's Graph steps — making per-file commit/step time-travel cheap.
- **Pure Rust search (`ignore` + `grep-searcher` crates)** can power `fzf`-style file indexing and `ripgrep` content search over `lf` RPC on both local and remote Machines without requiring external binaries on remote hosts.

## External agent-native file browsers

- **T3 Code (`pingdotgg/t3code`):**
  - `AnnotatableCodeView` lets you select lines in both the file browser and diff view to attach review comments; comments turn into structured context chips in the agent composer and survive tab/thread switches.
  - Opened files and diffs live as first-class tabs beside the conversation stream.
  - Diff scopes separate Uncommitted, Latest turn, specific Turn, and Commits.
  - Pain point observed in issues: fixed-width tree sidebars clip deep paths when horizontal scroll or compaction is missing.
- **Paseo (`getpaseo/paseo`):**
  - Daemon-backed workspace (local or remote VM) with a unified tabbed surface holding open files, diffs, and rendered previews (HTML, SVG, PDF, dev-server URLs) at one level instead of tabs nested inside a tab.
- **Orchestrator Inc (`orchestrator.inc`):**
  - Worktree-scoped inspector per worker session: changed files, live preview, and review threads sit beside the worker's chat/TUI, routing review annotations back to the owning agent.
- **Rune (`rune.build`):**
  - Unified keyboard-first command prompt + consistent modal navigation across editor, terminal, and file explorer (`fexplorer`, an editable text buffer where renaming/deleting/re-indenting lines applies bulk file ops on save).
  - Read-only doc snapshots open beside code without leaving the app.
- **Cursor (Agent Mode / 3.x):**
  - Separated agent edits out of chat threads into dedicated Changes / Commits / Reviews surfaces, plus visual click/drag annotation on rendered previews to send structural context back to the agent.
  - Cautionary pain point: Cursor's "jumping focus" bug (auto-advancing selection after an action/update) is a top user complaint — mirrors Loopflow's auto-scratch focus stealing.
- **Zed / Conductor:**
  - Highlight the "sidebar-in-sidebar" squeeze when an app already has a left workspace list + conversation pane + right inspector; responsive auto-dock/overlay breakpoints matter at half-screen widths.

## What to move from Loopflow's `Diff` into `Files`

Current `Diff` (`TaskChangesView.swift`, `ReviewStreamView.swift`, `ReviewDiff.swift`) already implements:
- **Line selection + inline comments (`ReviewSelection`, `ReviewComment`, `ReviewCommentEditor`, `ReviewCommentsStrip`):** Click or Shift-click line numbers to leave comments anchored to line text, batch them in `TaskFilesStore.comments`, and drop all of them into the conversation input via `⌘↩ Send to conversation` (or insert a bare line reference).
  - *Transfer to `Files`:* Share the exact same `TaskFilesStore.comments` queue and `ReviewCommentsStrip` in `Files` `Read` mode. This also fixes today's dead `Read / Edit` split on code files: `Read` renders syntax-colored, line-numbered, commentable code (and rendered Markdown/HTML), while `Edit` switches to `NSTextView`.
- **Responsive sidebar container (`ChangedFilesSidebar` in `TaskChangesView.swift:30`):** Auto-docks at `>= 620pt`, becomes a clean slide-over drawer below that, collapses after selecting a file when narrow, uses a single `sidebar.left` toggle with no separate `.dropdown` vs `.pinned` pin button, and wraps/subtitles directories cleanly.
- **`ViewThatFits` adaptive toolbar:** Collapses controls gracefully into `…` on narrow panes instead of truncating or stacking extra chrome bars.
- **Keep in `Diff` only:** Step selector (`Whole Task` vs commits), `Tour` ordering, `Viewed` checkmarks, `New code / Unified / Side by side` layout picker, and PR `DeliveryCard`.

## Why auto-scratch selection flashes today

Traced in `swift/LoopflowMac/TaskFileDocument.swift` (`415-467`, `749-765`, `882-886`) and `swift/LoopflowMac/Views/TaskFilesView.swift` (`476-522`):
1. `autoSelected` stays `true` until a user clicks inside `Files` (`guard !autoSelected, let lastInteraction else { return .select }`). If an agent writes `scratch/plan.md`, `scratch/questions.md`, then updates `scratch/plan.md`, every write returns `.select` and ping-pongs the view.
2. When `autoSelected == true` or `lastInteraction >= 120s`, any write under `scratch/` forces `page = .files`, yanking the user off `Brief` or `Diff`.
3. `TaskHTMLView` runs `.task(id: html) { await show() }` on every 150ms FSEvents debounce (`invalidate`), calling `WebPage.load(html:baseURL:)` repeatedly during rapid multi-edit turns and flashing white/blank while reloading.
4. `isScratchPage` treats every `.md`/`.html` under `scratch/` (including internal working notes like `scratch/questions.md`) identically to a finished review page like `scratch/explore.html`.

## Why nav/tabs feel awkward today

Four competing navigation layers exist at once:
1. Pane strip (`WorkspacePanesView`): `Brief | Diff | Files` (+ `Task & workflow`, `lf process`, URL tabs).
2. `documentRow` (`TaskFilesView`): a hidden dropdown `Menu` (`filename 1 of 3 ▾`) instead of visible open files, plus `sidebar.left`, Save, and `Read | Edit | Diff` (even though `Diff` is already a top-level tab and `Read`/`Edit` are identical on code).
3. Static non-interactive `location` bar (`This Mac  loopflow.xxx/dir`).
4. Navigator's 3-state dance (`.closed` → `.dropdown` floating overlay → `.pinned` split) with search limited to expanded folders.

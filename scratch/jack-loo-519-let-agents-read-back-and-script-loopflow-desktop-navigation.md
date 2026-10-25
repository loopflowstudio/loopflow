# LOO-519 — Agents read back and script Desktop navigation

Status: **accepted for implementation** (2026-10-24). Jack Heart's concept review
(2026-10-24) set the scope: **A**, a Rex-aligned `lf desktop` CLI, plus **B**, designated
session/shell/browser panes, freely arrangeable, with tabs in shell and browser.
Jack waived design review; kickoff settled the open questions (`scratch/questions.md`)
and Jack judges them in the demo. LOO-576 owns the sidebar scope. History: Jack Heart's
October 16 navigation brief (Task 2), moved from LOO-497 on October 18; LOO-568's
follow-ups folded in on October 24.

## Problem

An agent arranging Jack's work through Desktop cannot fully verify what it did,
cannot tell "no such pane" from "Desktop unreachable" by exit status, cannot
drive the window-level gestures a person uses (back, ⌘n, tile zoom), and from
inside a Desktop Session it has no caller identity, so every command needs
`--pane`. Pane identity is also weak: a Task workspace is a free split tree of
anonymous panes (`empty`, `session`, `shell`, `files`, `log`), so "the shell" or
"the browser" is not a target anyone can name. The Dev targeting shipped in #1664
has never been seen working.

## Demo

From a shell tab in **Loopflow Dev**, Task LOO-519 shown, Jack watching:

```sh
lf desktop ls                                  # Working Tasks: 1 LOO-519 …, 2 LOO-576 …; panes session shell browser
lf desktop do task.goto index=2                # what ⌘2 does
lf desktop do navigate.back                    # what ⌘[ does
lf desktop run -- cargo test -p loopflow desktop   # a new shell tab runs it, focus stays put
lf desktop capture -b shell:2 --trim | tail -3
lf desktop do browser.open url=http://localhost:5173
lf desktop move -b browser --destination session --axis vertical
lf desktop zoom -b browser --on
lf desktop state --json | jq '.windows[0] | {destination, history}'
lf desktop zoom -t 99; echo $?                 # 3: no Task 99, nothing changed
LF_DESKTOP=com.example.none lf desktop ls; echo $?   # 4: Desktop unreachable
```

Then, in the session pane, Jack asks the agent to "zoom yourself": it runs
`lf desktop zoom --on` with no `-b` and its own session pane fills the Task.
Same demo, from LOO-568: `lf desktop ls`/`focus` from the Dev Shell act on Dev,
not the installed app; Jack opens a conversation inline (#1580); Jack judges the
Dashboard tile's doubled border.

## Concepts (Rex mapping)

Reference: https://www.superlogical.com/rex/docs/automate/cli. Defer to Rex's
API where a verb exists; Loopflow's work model sits above it. Being more
constrained than Rex is fine.

| Rex | Loopflow | Target flag |
|---|---|---|
| session | Task (its workspace) | `-t/--task` |
| window (tab of a session) | — a Task has one workspace | none |
| block | designated pane, or `pane:N` tab | `-b/--block` |
| client | Desktop window | `--window` |
| — | Wave, Dashboard, Working Tasks | `view`, `query`, `do dashboard.*` |

Every target resolves in Rex's order, stopping at the first kind that matches:
full ID, case-insensitive label, 1-based position, ID suffix (≥4 chars).
Ambiguous or absent → change nothing, list matches, exit 3.

- **Task** (`-t`): label = issue key or name; position = place in the window's
  sidebar list (⌘3 ≡ `-t 3`). Root `--task/--wave/--repo` keep meaning Work
  selection; `-t` wins when both are given.
- **Pane** (`-b`): label = `session`, `shell`, `browser`; `shell:2` names a tab by
  position; position = order in the layout; full pane/tab IDs from `state` still work.
- **Window** (`--window`): position counts the repository's windows, frontmost first.

**Context.** A bare command acts on the caller, like `REX_BLOCK`: `LF_TERMINAL_ID`
names the terminal (session pane or shell tab) the command came from, trusted
from the environment without the PTY check, so an agent's tool subprocess gets
its own pane with no new variable. A stale inherited ID resolves to its pane, or
exits 3 if gone. Without a caller: root Work flags, then the checkout's Task
binding; window-level actions use the single window showing that Work. Nothing
ever follows focus; more than one candidate exits 3.

## Designated panes (B)

A Task workspace has exactly three panes: **session**, **shell**, **browser**.
They are always retained; the arrangement is free — `move`, `resize`, `zoom`,
`hide`, `restore` act on them, and the layout is a split tree over the visible
ones. Every visible pane has a Work owner by construction.

- **session** — one conversation, no tabs. It shows the Task's current
  conversation; other conversations of the Task switch into it
  (`do session.switch session=…`, palette, inline open from #1580). Desktop retains
  switched-away surfaces; nothing is closed.
- **shell** — tabs. `lf desktop run [-- CMD]` adds a tab at the Task's recorded
  location and selects it without moving focus (Rex's `--focus=false` as the
  default for scripts); ⌘T, "new terminal", adds and focuses one. One visible
  shell at a time.
- **browser** — tabs: `files` (the Task's Files view, which keeps its own document
  tabs and drafts), `graph-log`, and URL tabs (`WKWebView`; http/https only,
  back/reload, no address bar beyond the URL). Files and Graph log stop being pane kinds.

Wave and repository workspaces have only the session pane (shell and browser
already require a Task). `empty` panes and free splitting are gone.

## Command surface (A)

| Command | Rex | Replaces |
|---|---|---|
| `ls [--window W] [--json]` | `ls` | — (projection of the same inspection as `state`) |
| `state [--json]` | `inspect` | unchanged name; the complete test oracle |
| `do ACTION [k=v…]` | `do` | `enter`, `back`, `forward`, `slot`, `next`, `previous`, `files`, `graph-log`, `view working-tasks` |
| `actions [--json]` | `rex -C x actions` | `supported_operations` |
| `send [TEXT]` (stdin if omitted) | `send` | `text` |
| `send-key KEY` | `send-key` | `key` |
| `capture [--trim] [--max-bytes N]` | `capture` | `read` |
| `run [-- CMD]` | `run` | `shell` |
| `zoom [--on\|--off]` (bare toggles) | `zoom` | `zoom [--off]` |
| `focus`, `hide`, `restore`, `move`, `resize` | — | same, `-b` instead of `--pane` |
| `view`, `query`, `pin`, `unpin`, `rename`, `arrange`, `slots`, `order`, `layout` | — | unchanged (Loopflow layer) |

All pane commands take `-t`, `-b`, `--window`, `--json`. `--pane` is deleted.
Richer key names (`ctrl+c`) stay with LOO-452; `send-key` keeps today's key set.

**Actions are the one mutation path.** A Swift `DesktopAction` registry names
every gesture once — name, title, args, default key — and the shortcut monitor,
menus, Keyboard Shortcuts window and Apple event all call its `perform`. Verbs
are CLI sugar for actions (`zoom` → `pane.zoom`, `run` → `shell.new`). Inventory:

| Action | Args | Bound to |
|---|---|---|
| `navigate.back` / `navigate.forward` | — | ⌘[ / ⌘], crumb chevrons |
| `task.goto` | `index` | ⌘1–9 |
| `task.next` / `task.previous` | — | ⌃⇥ / ⌃⇧⇥ |
| `dashboard.working_tasks` | — | sidebar menu, crumb |
| `dashboard.enter` | `task` | tile click (only a member of the shown Dashboard) |
| `pane.focus` | `direction?` | ⌘⌥arrows; with `-b`, that pane |
| `pane.zoom` / `pane.hide` / `pane.restore` | `on?` | zoom key, close, undo close |
| `pane.move` / `pane.resize` | `destination, axis` / `toward, ratio` | drag |
| `pane.send` / `pane.send_key` | `text` / `key` | — |
| `shell.new` | `command?` | ⌘T |
| `shell.toggle` / `browser.toggle` | — | existing toggles |
| `browser.files` / `browser.graph_log` / `browser.open` | — / — / `url` | menus |
| `session.switch` | `session` | palette, inline open |

Chrome (sidebar toggle, palette, themes) stays out. Positions follow whatever list
the sidebar shows; LOO-576 changes that list, not this API.

## Readback

`state --json` (one `inspect desktop` Apple event) carries, per window:
`destination` (`work`, `dashboard`, `working_tasks`, `view`, `zoomed {from, work}`),
`history {back, forward}`, `list` (the sidebar's Tasks in ⌘n order: position,
work key, label), `actions` (descriptors), and per Task workspace its three
designated panes (`id`, `kind`, `visible`, `focused`, `zoomed`, `tabs`,
`selected_tab`, owner) plus the split tree. `ls` renders the same reply: header
naming the window and its scope, then positions, short IDs, labels and panes.
`ls` scope is the caller's window, else the repository's frontmost window, named
explicitly in the header.

## Exit statuses

0 success · 1 failure · 2 bad arguments, malformed expression, unknown action
or bad action args · 3 missing/ambiguous Task, window, pane or tab (nothing
changed) · 4 Desktop unreachable (not running, not macOS, Automation consent
pending -1712 or denied -1743, which names System Settings › Privacy & Security ›
Automation) · 130 interrupted. Typed errors decide the status: Swift sets
`scriptErrorNumber` 1002 (argument) / 1003 (target); Rust reads osascript's
`(NNN)` suffix. A Desktop older than this lf (`-1708`, event not understood)
exits 1 naming the fix: update Desktop.

## Contract

- **Source of truth.** Persisted arrangement is `work_pane_layouts.layout_json`
  through the Rust store (`WorkPaneLayout`); live destination, focus, zoom and
  retained surfaces are the window's `WorkNavigation`/`SessionsWorkspaceRegistry`.
  `state` reads both through `inspectDesktop`; the CLI caches nothing. An action
  receipt carries the destination and pane state the next `state` reports.
- **Persisted shape.** `PaneIntent` becomes `Session | Shell{tabs,selected} |
  Browser{tabs,selected}`; `BrowserTab` is `Files{tabs,selected} | GraphLog |
  Url{url}`. Validation: Task layouts hold exactly one of each kind (tree ∪
  hidden), Wave/repository layouts exactly one session. A draft migration
  converts stored layouts in Rust (precedent: `convert_project_content`): Files
  and Log panes merge into one Browser pane at the first one's place; empty
  panes collapse; extra Session panes leave the layout (their conversations stay
  switchable); missing designated panes are added hidden. File drafts survive unchanged.
- **Wire DTOs.** `DesktopWindowInspection` gains `list`, `actions`; pane
  inspection gains `kind` and designated tabs; `supported_operations` goes.
  `DesktopActionRequest {repository, window, task?, pane?, action, args}` and its
  receipt replace `DesktopPaneCommand` and `DesktopNavigationCommand`. No
  defaults; fixtures `desktop_inspection.json`, `desktop_action.json` with Rust
  and Swift fixture tests. The Swift registry's names equal the fixture's action
  list (Swift test); every Rust verb's action is in it (Rust test).
- **Absent states.** No window for the repository → 3. Task without a
  conversation → its session pane shows the start state; `send` to it exits 3.
  `capture`/`send` on a browser tab → 1, "not a terminal".

## Current → target architecture

| | Before | After |
|---|---|---|
| Pane model | free tree; `empty/session/shell/files/log` | three designated panes; browser tabs |
| Mutations | `CNRTpane` (11 typed kinds) + `CNRTnavg` + `SessionsShortcut` + menu closures | one `DesktopAction` registry, one `perform desktop action` event |
| Gestures from CLI | per-gesture verbs | `do ACTION k=v` |
| Caller | `LF_TERMINAL_ID` + PTY check; agents pass `--pane` | env trusted for targeting; PTY check stays for Session attachment (`current_terminal_id`, #1386) |
| Targets | exact pane IDs | Rex resolution for `-t`, `-b`, `--window` |

## Delete — do not maintain

Done in slices 2–3 except where noted; kept here for review.

- `PaneContent.empty/.files/.graphLog`, `PaneIntent::Empty/Files/Log` and free
  split creation (`newTerminal` as a split; `.empty` handling in views).
- `SessionsShortcut` and its handler (`SessionsView.swift`); the
  `WorkModel.navigate`/`DesktopNavigationAction` dispatch (its bodies move into
  action handlers); `CNRTnavg` (`Loopflow.sdef`, `NavigateDesktopCommand`);
  `DesktopPaneAction`/`DesktopPaneCommand` (Rust, Swift) and the `CNRTpane` event.
- CLI: `text`, `key`, `read`, `shell`, `files`, `graph-log`, `enter`, `back`,
  `forward`, `slot`, `next`, `previous`, the `working-tasks` view expression, `--pane`.
- `supported_operations`; `selection_kind`/`selection_id` (already gone in the diff).
- `tests/fixtures/dto/desktop_navigation.json`, `desktop_pane_commands.json`, `desktop_pane_receipt.json` (→ `desktop_action.json`).
- `WorkNavigationShortcut.task/.adjacent` (keys moved to the registry).
- `docs/lf.md` sections teaching `--pane`, the nav verbs, and Files/Graph log panes.

## Forbidden outcomes

A verb that mutates outside the action registry; a second readback beside the
inspection (`ls` is a rendering of it); an `empty` or fourth pane kind; Files or
Graph log as panes again; keeping `--pane` beside `-b`; target resolution that
falls back to focus; exit codes chosen by matching message text; a layout reader
that accepts both shapes.

## Carried from the stopped pursue (committed in 56ffb2c6f3)

In place: `DesktopError` (Target 3 / Unreachable 4) and osascript code mapping;
Swift `DesktopTargetError` → 1003; `destination`/`history` in state, Rust/Swift
DTOs and fixture; `after_help` exit text and the AGENTS.md rule; navigation
handler bodies in `WorkModel`; `WorkDestination`/workspace test updates. Agent
`LF_AGENT_CALLER` Session inference is deleted (e5ff80c882).
Still to do in slices 2–3: those `WorkModel` bodies become `navigate.*`,
`task.*`, `dashboard.*` action handlers; `CNRTnavg` folds into the generic action
event; the nav verbs (`enter`, `back`, `forward`, `slot`, `next`, `previous`) and
`view working-tasks` go; the docs navigation section is rewritten.
`.lf/wave/product/MEMORY.md` was compressed by that run; left as is.

## Slices

One PR; deepest deletion first.

1. **Designated panes — implemented 2026-10-24.** Rust `PaneIntent` is
   `Session{session_id?} | Shell{tabs, selected?} | Browser{location, tabs, selected?}`,
   `tree` is required, and validation demands exactly one of each kind (Task) or one
   session (Wave/repository). Draft `designated_panes` marks the batch
   (`-- breaks:`); `convert_released_data` recognizes its statement after released
   migrations and drafts alike and runs `convert_released_layouts`. Swift `PaneContent` is `session(id?) | shell | browser`;
   `MultiplexerStore(taskId:)` builds the designated tree with stable pane IDs
   `session`, `shell`, `browser` (migrated layouts keep theirs). Close hides a
   pane; ⌘W closes the selected shell/browser tab and the last tab hides the pane.
   Opening a conversation switches the session pane in place. `BrowserPageView`
   is a bare `WKWebView`. Still open here: browser back/reload controls; the
   current `desktop shell` verb already keeps focus (as `run` will).
2. **Action registry — implemented 2026-10-24.** `LoopflowMac/DesktopActions.swift`
   holds every action (name, title, scope, args, default `DesktopKeyBinding`s,
   `available`, `perform`); `DesktopActionMonitor`, View/File menus, Keyboard
   Shortcuts and `PerformDesktopActionCommand` (`CNRTactn`) call it.
   `WorkModel.navigate`/`selectWorkingTask`/`cycleWorkingTask` bodies moved into
   `navigate.*`/`task.*`/`dashboard.*`; `WorkNavigationShortcut` keeps only ⌘K/⌘\.
   `DesktopArgumentError` → 1002. Added beyond the inventory: `pane.close` (⌘W:
   selected tab, last tab hides) and `pane.reopen` (⇧⌘T: undo close); `pane.restore`
   shows the addressed hidden pane. Tabs inspect as `DesktopTabInspection` (shell and
   browser). `DesktopPaneControlTests.everyKeyBindingMatchesItsAppleEvent` performs
   each binding by key and by event on twin fixtures and compares the results.
3. **CLI — implemented 2026-10-24.** `ls`, `state`, `actions`, `do`, `send`,
   `send-key`, `capture [--trim]`, `run [-- CMD]`, `zoom [--on|--off]`, `focus`,
   `hide`, `restore`, `move`, `resize`, all with `DesktopTargetArgs` (`-t`, `-b`,
   `--window`, `--json`). `Scope` resolves window → workspace → pane in Rex order
   (`resolve`); `LF_TERMINAL_ID` is read directly. `DesktopError::Argument` (2) and
   `-1708` → "update Desktop". `VERB_ACTIONS` maps verbs to actions; a unit test
   checks each is in the fixture. Docs (`docs/lf.md`, `swift/README.md`,
   architecture reference) rewritten; reference regenerated.
   Still open: browser back/reload controls (slice 1 carry-over).
4. **Demo on Dev** (Jack).

## Done when

- `lf desktop ls` in a Dev Shell lists Dev's sidebar Tasks with positions matching
  ⌘n and each Task's session/shell/browser; `state --json` reports the same.
- `lf desktop do task.goto index=2`, `navigate.back`, `dashboard.enter task=…`
  produce exactly what ⌘2, ⌘[ and a tile click produce, and the receipt's
  destination equals the next `state`.
- `lf desktop run -- CMD` adds a shell tab running CMD; `capture -b shell:N`
  returns its output; `do browser.open url=…` shows the page in a browser tab.
- `move`/`resize`/`zoom`/`hide`/`restore` rearrange the three panes, and a
  relaunch restores the arrangement; an existing layout with Files and Log panes
  opens as one browser pane with those tabs and its file drafts intact.
- An agent in the session pane runs `lf desktop zoom --on` with no `-b` and its
  own pane zooms.
- `-t 99` → 3, nothing changed; ambiguous label → 3 listing matches; unknown
  action → 2; Desktop not running or consent pending → 4.
- From a Dev Shell, `lf desktop focus` moves focus in Dev, not the installed app (Jack, demo).

## Checks

Gate, headless, inherited `LF_*` unset, `< /dev/null`:

- `cargo test -p loopflow --lib desktop` and `--lib selection` — resolution,
  caller trust, classifier, layout validation.
- `cargo test -p loopflow --test integration dto_fixtures::desktop` and the
  layout migration test; Swift `DTOFixtureTests`.
- `scripts/test_desktop.sh --filter 'DesktopPaneControlTests|DTOFixtureTests|WorkspaceRetentionTests|DashboardWorkspaceTests|WorkDestinationTests|WorkOpeningCLITests'`.
- `UPDATE_CLI_REFERENCE=1 cargo test -p loopflow --test integration reference_matches_the_compiled_cli`; `cargo clippy --all-targets -- -D warnings`.

Demo/review (Jack): Dev targeting, inline conversation, doubled frame, pane feel.

## Exclusions

Sidebar scope and header animation (LOO-576); readiness waits, `run --wait` and
events (LOO-453); richer keys (LOO-452); OSC 7501 (LOO-422); external hosts
(LOO-498); `LF_DESKTOP=1` removal (LOO-571); side-by-side shells; flattening
Files' document tabs into browser tabs; Lua, daemons, cwd/cross-repo pane moves;
the doubled frame unless Jack picks a fix in the demo.

## Risks

- **Layout migration is a released-store change**: a branch build that opens
  Jack's lfdb raises the reader floor. Dev must run against a copied store until
  release, per the migration-compat practice.
- **Trusting `LF_TERMINAL_ID`** lets an inherited marker target a pane; the
  blast radius is one Desktop pane of the same user, and a gone pane exits 3.
- **Dev Automation consent**: LOO-568's headless check reached Dev but got
  "AppleEvent timed out", likely consent pending (-1712 → exit 4). The demo
  grants consent first; until then Dev targeting is unproven.
- **`WKWebView` on macOS 15** needs no deployment-target change; SwiftUI
  `WebView` would need 26 and is not used.

Check results (2026-10-24, implement slices 2–3): `cargo test --lib commands::desktop selection_ designated_panes` 50 pass; integration `dto_fixtures::desktop cli_discovery::desktop structured_help reference_matches` pass; clippy clean; Swift 14 focused suites, 242 pass. Full Rust integration (`work_watch` etc.) deferred to gate.

Compress (2026-10-24): `Verb` carries `--json` in its target args; pane resolution builds candidates through one constructor and indexes positions directly; unused `DesktopKeyBinding.matches(_:)` deleted. `cargo test --lib commands::desktop` 9 pass, clippy clean, Swift `DesktopPaneControlTests|WorkNavigationTests` 63 pass.

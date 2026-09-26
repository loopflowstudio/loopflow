# State of the art vs. the Loopflow workspace

Research for LOO-291, 2026-09-26. Jack's verdict on the native build: "looks a lot
worse than the mocks. Plus I bet we can even improve on the mocks." Target: at least
as good as Linear, Notion, Figma, Claude, OpenCode and Codex. Scope this pass: light
mode only.

Every product claim carries a source. `[obs]` marks my own observation of a capture
or of this repo. `[3rd]` marks a third-party scrape (values inferred, not official).
`unverified` means no public source was found; the claim should not be repeated as fact.

Captures reviewed: `scratch/demo-ready-evidence/configured-product-after.png` (installed
build, live data), `scratch/visual-study/polish/native/*.png` and `native/pass2/*.png`
(branch build, fixture data), and the D mock (`scratch/visual-study/polish/polish.css`,
`polish.js`).

## 0. What the captures show `[obs]`

| Surface | Mock D | Native branch build | Installed build |
|---|---|---|---|
| Sidebar | 264px, serif Wave 19, Task 13.5, count column, search at bottom | Same rules; fixture has one Wave | Waves in serif with a large filled state circle and a chevron; Tasks in sans with a small dot; then nine orphan Sessions in a two-line row style, each subtitled "Repository or unavailable ancestry"; search at bottom |
| Wave page | 48px serif title, 18px serif objective, tinted notice, caps headings, plan panel | Matches | 46px serif title; a 7-line objective in 18px serif dominates the fold; notice; KRs; 2 Tasks; "Wave controls" disclosure |
| Task page | 32px serif title, Flow panel, status, Sessions panel, Description, Comments | Matches; both loop return arrows land ~20px apart at `implement`; loop labels float above regions; 1px hairline connectors | Not captured |
| Session | Breadcrumb; dark panes with 12px radius and 12px gaps | Breadcrumb row + pink worktree strip (folder, name, 4 icons) + per-pane header (dot, name, ?, 4 icons) + floating green "Complete" pill; blue focus border | Not captured |

The mock never rendered orphan Sessions, a long objective, or the worktree strip, which
is most of why "the native build looks worse": the fixtures hid the three loudest
things. The branch navigator (`swift/LoopflowMac/Views/WorkspaceNavigator.swift`)
draws Waves with a chevron and no state circle and Tasks with a 7pt dot, so the large
Wave circles Jack saw belong to the installed build's older row. The orphan rows come
from `WorkspaceProjection.outline` (`WorkspaceProjection.swift:197`), which appends
`unmatchedSessions` at depth 0 after all Waves with that fallback string whenever
`session.workPath` is nil. The Session chrome is three components each owning a full
split/close set: `WorktreeNodeView` (`SessionsView.swift:551`), `SessionPaneView`
(`:794`), and `WorkspaceBreadcrumbBar.swift`.

## 1. What makes each one special

### Linear
- Theme is generated from three variables (base, accent, contrast) in LCH; chroma on
  neutrals is deliberately limited "to achieve timelessness"; the same generator yields
  high-contrast presets. https://linear.app/now/how-we-redesigned-the-linear-ui
- Inter Display for headings, Inter for body; 2024 redesign darkened light-mode text.
  Same source.
- March 2026 refresh: "Navigation sidebars are slightly dimmer, allowing the main content
  area to stand out"; headers, nav and view controls unified across entity types.
  https://linear.app/changelog/2026-03-12-ui-refresh
- Two-state lists: J/K *highlight*, X *selects*; ⌘K acts on the selection.
  https://linear.app/docs/select-issues
- No spinners: local store first, background sync, transitions mostly under 150ms.
  [3rd] https://performance.dev/how-is-linear-so-fast-a-technical-breakdown
- Agent vocabulary: activities `thought | elicitation | action | response | error`;
  session states `pending → active → awaitingInput | error | complete | stale`; first
  activity within 10s or the session is marked unresponsive.
  https://linear.app/developers/agent-interaction
- Sidebar: Inbox, My issues, Favorites (appears only once you favorite something),
  Teams; unread shown as count *or* dot, user's choice; rarely used items behind More.
  https://linear.app/changelog/2024-12-18-personalized-sidebar https://linear.app/docs/favorites
- Cycle graph: scope grey, target dotted blue, started yellow, completed solid blue;
  started work counts 25%. https://linear.app/docs/cycle-graph
- Karri: design system is "mostly colors, type and some basic components"; "the real
  design is the app". https://x.com/karrisaarinen/status/1715085201653805116

### Notion
- One row style, true page tree, sections Favorites / Teamspaces / Shared / Private that
  collapse on click; hover reveals `+` and `•••`; a page with no title renders grey
  "Untitled". https://www.notion.com/help/navigate-with-the-sidebar
  https://www.notion.com/help/customize-and-style-your-content
- Only two badges anywhere in the sidebar: a red Inbox badge and a blue dot on an unread
  chat. Same source.
- Warm near-black text `rgba(0,0,0,.95)`, `1px rgba(0,0,0,.1)` borders, 8px base,
  radii 4/8/12, shadows ≤ 5% opacity. [3rd] https://open-design.ai/plugins/design-system-notion/
- Slash menu is the command surface (`/h1`, `/todo`); ⌘K and ⌘P both search.
  https://www.notion.com/help/keyboard-shortcuts
- Custom Agents: an "Agents" sidebar section; each agent page is Chat / Activity /
  Settings; every run logged, every change reversible.
  https://www.notion.com/help/custom-agents

### Figma
- UI3: toolbar moved to the bottom; ~200 hand-drawn icons; rounded corners introduced;
  inputs got backgrounds. https://www.figma.com/blog/behind-our-redesign-ui3/
- Floating panels shipped, then were reverted to fixed resizable panels because they
  "cramped the canvas"; floating survives only in Minimize UI (`Shift+\`).
  https://www.figma.com/blog/our-approach-to-designing-ui3/
- UI font is Inter, which was designed for 11px on Figma's own interface.
  https://rsms.me/work/inter/ Exact UI3 sizes: unverified.
- Prototype "noodles" are hidden unless the source is selected because "the more
  interactions… the harder it becomes to interpret what's going on".
  https://www.figma.com/blog/prototyping-updates-and-interactive-components/
- Left panel is one linear list: file, branch, project, then pages and layers; a layer
  tree with a type icon per row. https://www.figma.com/blog/our-approach-to-designing-ui3/
  https://help.figma.com/hc/en-us/articles/360039831974
- Quick actions `⌘/` open centered with recent actions first.
  https://help.figma.com/hc/en-us/articles/360040328653
- Presence: every collaborator's cursor and selection are shown "because it provides
  important context"; click an avatar to follow.
  https://www.figma.com/blog/multiplayer-editing-in-figma/

### Claude (claude.ai, Desktop, Claude Code)
- Brand: Styrene for headings, Tiempos for body, a colour system chosen "to bring
  warmth". https://geist.co/work/anthropic https://type.today/en/journal/anthropic
- In the product: "UI elements and user messages are sans serif, chat title and
  assistant messages are serif". https://news.ycombinator.com/item?id=41917027
  Captured tokens: canvas `#FAF9F5`, hairline `#DEDCD1`, coral `#D97757`, ui-label
  0.75rem/500. [3rd] https://design.withfudge.com/tokens/claude.ai
- Claude Code desktop: sidebar lists sessions, filter by status/project/environment,
  group by project; every pane (chat, diff, terminal, tasks, subagent) has a draggable
  header; transcript has three tiers, Normal / Thinking / Verbose, cycled with `Ctrl+O`,
  where Normal means "tool calls collapsed into summaries"; OS notification when a
  session finishes off-screen. https://code.claude.com/docs/en/desktop
- Claude Code TUI: one accent token drives the spinner and the assistant label
  (`claude`, `claudeShimmer`). https://code.claude.com/docs/en/terminal-config
  Collapsed tool output reads `… +15 lines (ctrl+o to expand)`.
  https://github.com/anthropics/claude-code/issues/91827
- Agent view rows are grouped Pinned / Ready for review / Needs input / Working /
  Completed; each row is glyph + name + one-line status + age or `#PR`; PR label colours
  yellow / green / purple / grey. https://code.claude.com/docs/en/agent-view
- Renderer is a forked Ink with a ~16ms frame budget and synchronized output to kill
  flicker. https://news.ycombinator.com/item?id=46701013
- Permission prompt: Yes / Yes-and-don't-ask / No, `Tab` to add a comment.
  https://code.claude.com/docs/en/permissions

### OpenCode
- Theme roles sit on a 12-step neutral ramp: background = step 1, backgroundPanel = 2,
  backgroundElement = 3, borderSubtle = 6, border = 7, borderActive = 8, primary = 9,
  textMuted = 11, text = 12. Dark ramp `#0a0a0a #141414 #1e1e1e …`; light ramp
  `#ffffff #fafafa #f5f5f5 #ebebeb …`.
  https://github.com/anomalyco/opencode/blob/dev/packages/tui/src/theme/assets/opencode.json
- 15 core roles (primary/secondary/accent, error/warning/success/info, text/textMuted,
  three backgrounds, three borders) plus diff, markdown and syntax roles.
  https://opencode.ai/docs/themes/
- Tool state is `pending | running | completed | error`; `/details` and `/thinking`
  toggle disclosure; leader key `ctrl+x`, 2000ms timeout.
  https://github.com/anomalyco/opencode/blob/dev/packages/tui/src/routes/session/index.tsx
  https://opencode.ai/docs/tui/ https://opencode.ai/docs/keybinds/
- Footer: cwd left; right side `• N LSP` with the dot coloured `success` or `textMuted`.
  https://github.com/anomalyco/opencode/blob/dev/packages/tui/src/routes/session/footer.tsx
- No orphan section: sessions belong to a project directory; titles default to the
  truncated first prompt. https://opencode.ai/docs/cli/

### Codex (CLI + app)
- TUI style guide: green = success/additions, red = errors/deletions, magenta = Codex
  elements, cyan = tips; "most of the time, just use the default foreground".
  https://github.com/openai/codex/blob/main/codex-rs/tui/styles.md
- Working row: `• Working (12s • esc to interrupt)` with a 2-second shimmer sweep;
  elapsed formatted `12s`, `1m03s`.
  https://github.com/openai/codex/blob/main/codex-rs/tui/src/status_indicator_widget.rs
  https://github.com/openai/codex/blob/main/codex-rs/tui/src/shimmer.rs
- Tool cells: `• Running <cmd>` → `• Ran`, `Explored` with `└ Read <file>` children,
  failure `✗ (1) • 0ms`. https://github.com/openai/codex/blob/main/codex-rs/tui/src/exec_cell/render.rs
- Approval: numbered options with single-letter keys and a `›` cursor.
  https://github.com/openai/codex/blob/main/codex-rs/tui/src/bottom_pane/snapshots/
- App: threads target Local / Worktree / Cloud; "Hand off" moves a chat between them;
  permanent worktrees become sidebar projects.
  https://learn.chatgpt.com/docs/environments/git-worktrees
- Sidebar: project rows → thread rows, switchable "By project" / "In one list"; users
  filed "Projects and Threads… nearly the same visual weight".
  https://github.com/openai/codex/issues/29161 Per-thread status glyphs are a request,
  not shipped. https://github.com/openai/codex/issues/41423
- Cloud task list grouped Today / Yesterday / Merged / Closed with `+N −M` counts.
  https://learn.chatgpt.com/docs/cloud

### Only where they teach something the six don't
- **Ghostty**: no per-split chrome; `unfocused-split-opacity` 0.7 default, `window-theme
  = auto` picks chrome from the terminal background, `macos-titlebar-style = tabs` paints
  the titlebar in the terminal colour. https://ghostty.org/docs/config/reference
- **Warp**: block = command + output; non-zero exit = red background and red left rule;
  active pane marked by a triangle; agent tab icons `working | blocked | completed |
  errored`; at most two toasts. https://docs.warp.dev/terminal/blocks/block-basics/
  https://docs.warp.dev/agents/capabilities/agent-notifications/
- **Zed**: `ui_font_size 16`, `buffer_font_size 15`, project panel `indent_size 20`,
  `default_width 240`; active pane border 0 and inactive opacity 1.0 by default; threads
  grouped under project section headers; worktree threads fold under the main project.
  https://raw.githubusercontent.com/zed-industries/zed/main/assets/settings/default.json
  https://zed.dev/docs/ai/parallel-agents
- **Raycast**: list row = icon + title + subtitle + right-aligned accessories; ⌘K action
  panel shows shortcuts on the right. https://developers.raycast.com/api-reference/user-interface/list
- **Things 3**: Inbox / Today / Upcoming / Anytime / Someday / Logbook; metadata hidden
  until needed. https://culturedcode.com/things/features/
- **Graphs**: GitHub Actions is a DAG with a status icon per job and no loops; n8n draws
  a loop as a literal back-edge; Unreal draws a loop as one node with `Loop Body` and
  `Completed` outputs so no back-edge exists; FigJam defaults to elbow connectors.
  https://docs.github.com/en/actions/how-tos/monitor-workflows/use-the-visualization-graph
  https://docs.n8n.io/build/flow-logic/loop
  https://dev.epicgames.com/documentation/unreal-engine/flow-control-in-unreal-engine
  https://help.figma.com/hc/en-us/articles/1500004414542

## 2. What they all agree on (the baseline Loopflow must meet)

| Baseline | Evidence |
|---|---|
| One UI sans at 11–13px for chrome, with a distinct display face only for content titles | Figma 11px Inter; Linear Inter Display headings; Claude serif for assistant voice only; Zed 16 is the outlier and is a settings default |
| Sidebar is dimmer than content, one tone step down, joined by a hairline | Linear 2026 refresh; Notion surface `#f6f5f4` on white [3rd]; OpenCode backgroundPanel = step 2 |
| Depth via a surface ladder and 1px hairlines, not drop shadows; shadows only on floating things | Linear "no drop shadows" [3rd]; Notion ≤5% [3rd]; OpenCode steps 1–3 |
| Neutral chroma limited; one accent; state colours reserved for state | Linear LCH post; Codex "default foreground"; Claude one `claude` accent |
| Counts only on inboxes; status on rows is a glyph, never a number | Linear count-or-dot on Inbox only; Notion two badges total; Figma one blue dot; Zed thread status indicator |
| Hover reveals row actions; selection ≠ keyboard highlight | Notion `+`/`•••`; Zed archive; Linear J/K vs X |
| ⌘K-class palette with shortcuts shown inline; single-key verbs in lists | Linear, Figma `⌘/`, Raycast, OpenCode `ctrl+p`, Notion `/` |
| No blocking spinners on navigation; last-good data stays on screen | Linear local-first; Notion SQLite cache https://www.notion.com/blog/faster-page-load-navigation |
| Running work = one animated row with elapsed time and an interrupt hint | Codex `• Working (12s • esc to interrupt)`; Claude `Befuddling… (1m 56s · ↑ 2.3k tokens)` https://github.com/anthropics/claude-code/issues/24688 |
| Tool/agent state vocabulary is four or five words | ACP `pending / in_progress / completed / failed`; OpenCode `pending / running / completed / error`; Warp `working / blocked / completed / errored`; Linear `active / awaitingInput / error / complete / stale` |
| Detail is collapsed by default with a visible expand affordance and a global disclosure toggle | Claude `Ctrl+O` tiers; OpenCode `/details`; Codex `… N bytes omitted` |
| Completion is pushed (OS notification), not badged forever | Claude Code desktop; Zed `notify_when_agent_waiting`; Warp toasts |
| Transitions 100–250ms, reduce-motion honoured | Linear [3rd]; Notion 150/200ms [3rd]; Claude `prefersReducedMotion` |

## 3. Where they disagree

| Axis | Side A | Side B | Trade-off |
|---|---|---|---|
| Panels | Fixed, resizable (Figma after reverting; Linear; Zed docks) | Floating (Figma UI3 initial; FigJam) | Floating gains canvas but occludes content; Figma users rejected it for a work surface |
| Unfocused panes | Dim them (Ghostty 0.7; iTerm2, Warp opt-in) | Leave them (Zed, VS Code defaults) | Dimming makes focus unmistakable at the cost of legibility in the other pane |
| Per-pane chrome | None; keyboard + context menu (Ghostty, Warp, Zed) | Always-visible pane header (Claude Code desktop; iTerm2 optional) | Headers make drag/reorder discoverable but stack with tabs and breadcrumbs |
| Sidebar hierarchy | Section header → flat rows, ≤2 levels (Linear, Codex, Zed threads, Claude Code) | One row style with a true tree (Notion, Figma layers, Zed project panel) | Trees suit documents; work tools flatten because status and recency matter more than nesting |
| Warmth | Warm cream + serif voice (Claude, Notion warm near-black) | Neutral greys, blues (Linear, OpenAI, OpenCode) | Warmth signals authorship and calm; neutral reads as tool and defers to content |
| Loops in a graph | Literal back-edge (n8n) | Loop as a node with body/done outputs (Unreal) | Back-edges show the real topology but cross; loop-nodes stay acyclic and readable |
| Command surface | Global palette (Linear, Figma, Raycast) | In-context slash menu (Notion, Claude Code, Codex) | Palette is location-free; slash menus are faster when the cursor is already in a text field |
| Terminal palette | Terminal inherits the app theme (VS Code, Zed, Warp, JetBrains) | Terminal keeps its own dark palette inside a light app (Claude Code desktop; users filed it as a bug https://github.com/anthropics/claude-code/issues/45115) | Inheriting removes the seam; a fixed dark terminal preserves the scrollback everyone knows |

## 4. What is different about Loopflow

None of the six had to solve all of these at once:

1. **Native terminals as first-class content, not a tool panel.** In Zed, VS Code and
   Claude Code desktop the terminal is a dock or a pane beside the real content. In
   Loopflow the Session *is* the terminal; the light workspace is the frame around it.
   Demand: the dark region must read as the page, not as a panel; the chrome above it
   must be the same chrome as the Wave and Task pages.
2. **A Flow with loops and occurrence identity.** GitHub Actions is a DAG; n8n and
   Unreal show loops but not per-occurrence completion or per-loop return counts.
   Demand: a linear-reading diagram that carries two return edges, an iteration tuple,
   and pass-scoped completion, without becoming a canvas.
3. **Sessions with membership in an execution step.** Codex threads target
   Local/Worktree/Cloud; Zed folds worktree threads under a project. Nobody labels a
   conversation "inside `feature / review-slice`, iteration (2, 1)". Demand: membership
   is a fact on the Session row and in the breadcrumb, distinct from "Independent",
   and clicking it lands on the exact node.
4. **Headless Runs alongside live Sessions.** Codex cloud and Claude agent view list
   runs, but as their own product surface. Demand: Recent Runs live under the Task, on
   demand, in the same tone vocabulary as Sessions.
5. **KR proofs and metric windows on the planning page.** Linear has cycle graphs and
   Notion has progress rings, but neither shows "held for a 7-day window, instrumented,
   fresh until". Demand: a reading, not a dashboard; the Metrics table row is right.
6. **Two authorities the UI must not merge.** Linear is the planning authority; Loopflow
   is the execution authority. Demand: the issue ID is a link out; Description is
   Markdown from Linear; nothing invents a second store.

## 5. What is essentially the same, and what to borrow

| Loopflow piece | Same as | Borrow exactly |
|---|---|---|
| Repository-headed sidebar | Linear teams / Zed threads-by-project | Section-header → flat rows; sidebar one tone darker; hover-only row actions; Favorites-style conditional sections |
| Wave page (objective → KRs → plan) | Linear project page / Notion page | Serif title once; caps 11 tracked section labels; plan rows as a bordered list with aligned columns; cycle-graph colour logic for KR state |
| Task plan rows | Linear issue list | Rank, state glyph, title, state chip, mono ID; J/K highlight vs selection |
| Session breadcrumb | Figma left panel (file › branch › project) / VS Code editor breadcrumbs | One linear identity slot; last crumb bold; ancestors quiet; membership as a mono chip |
| Session rows | Claude Code agent view / Zed thread rows | glyph + name + one-line status + provider + age; grouped by state when > 5 |
| Running status line | Codex status indicator / Claude Code spinner | `● review-slice · 12m · claude` with a 2s shimmer on the running node only |
| Collapsed tool detail (Recent Runs, Comments) | Claude Code `Ctrl+O` tiers / OpenCode `/details` | Chevron + caps title + count; one global "show details" toggle |
| Terminal panes | Ghostty (already libghostty) | No per-split chrome; dim unfocused 0.7–0.85; keybinds + context menu for split/close/zoom |
| Pane identity | VS Code terminal tabs | Show the tab strip only with ≥2 panes; a single pane's name moves into the toolbar |
| Flow diagram | GitHub Actions status-per-node + n8n back-edge + FigJam elbow connectors | Status icon left of the name; elbow return edges; label inside the region |
| Blocked Run | Warp failed block | Red left rule + tinted background + the reason + a help action |
| Command palette | Raycast / Linear ⌘K | Row = icon + title + subtitle + right accessory; shortcuts visible; recent first |
| Permission / confirm (Stop & restart) | Claude Code permission prompt | Question, consequence sentence, Cancel + one accent outline action, Escape cancels |

## 6. Sidebar: hierarchy, glyphs, orphans

Jack: the native sidebar is "very incoherent" — serif Waves with big filled state
circles, sans Tasks with small dots, then orphan Sessions in a different two-line row.
He asked whether the Wave circles should be "simple counts of open tasks or something".

| Product | (a) Row hierarchy | (b) Status / count glyphs | (c) Orphans |
|---|---|---|---|
| Linear | Flat sections (Inbox, My issues, Favorites, Teams) with team rows expanding to fixed sub-pages; hidden items behind More https://linear.app/docs/default-team-pages https://linear.app/changelog/2024-12-18-personalized-sidebar | Inbox only, count *or* dot by preference; no evidence of counts or health icons on team/project rows (unverified) | *Drafts* row appears only while drafts exist https://linear.app/changelog/2023-05-11-issue-drafts (via search summary, page not fetched); "No project" exists only inside grouped views https://linear.app/docs/display-options |
| Notion | One row style, true tree, sections collapse on header click https://www.notion.com/help/navigate-with-the-sidebar | Red Inbox badge and blue unread-chat dot; nothing on page rows | *Private* is the default landing; untitled pages render grey "Untitled" |
| Figma | File browser: Recents / Starred / Drafts / Browse; editor: one linear list, layer tree with type icons https://help.figma.com/hc/en-us/articles/360039831974 | One blue dot on Libraries for available updates | *Drafts* per team https://help.figma.com/hc/en-us/articles/18409526530967 |
| Claude / Claude Code | Flat Starred / Recents; Code tab groups sessions by project, filter by status https://code.claude.com/docs/en/desktop | No row status in claude.ai; Code tab has a Dispatch badge and OS notifications | Recents; ungrouped sessions stay in one list |
| Codex app | Project rows → thread rows, "By project" or "In one list"; permanent worktrees promoted to projects https://learn.chatgpt.com/docs/projects | Unread indicator on Automations only; per-thread status requested, not shipped https://github.com/openai/codex/issues/41423 | Standalone threads in Chats; threads without project metadata vanish from By-project view https://github.com/openai/codex/issues/34139 |
| Zed | Project panel: one row type, `indent_size 20`, sticky ancestors; threads grouped under project section headers https://zed.dev/docs/project-panel https://zed.dev/docs/ai/parallel-agents | Git-status tint on names (default on); numeric badges ship off; threads show status indicator + agent icon | Worktree threads fold under the main project; history is a separate chronological view https://zed.dev/docs/ai/agent-panel |
| OpenCode | No persistent list in the TUI (`/sessions` modal); desktop moved to tabs https://opencode.ai/docs/tui/ | None; community plugins add status icon + bold active row https://github.com/Leogimp/opencode-sessions-sidebar | None; sessions belong to a directory |

Patterns: work tools use section headers plus at most two flat levels; counts are for
inboxes; status is a glyph and only for states that need eyes; orphans live in a named
conditional section that disappears when empty, never a bucket keyed on a missing field;
worktree work folds under its parent; untitled things get auto-titles.

Recommendation for Loopflow's three cases:

1. **Waves.** No state circle and no count. Wave rows are section-like: Lato 13/600,
   ink-2, chevron on the left (always visible; Zed `folder_indicator`), the row itself
   selectable. This overrides D's serif 19 Wave names; serif stays on the repo name and
   page titles. If a roll-up is wanted, use one 6pt dot at the right in `human` yellow
   only when a child Task needs Jack (Warp's tab icon, Codex's unread), otherwise nothing.
   Counts of open Tasks answer a question nobody asks in a sidebar and would make every
   Wave row look like an inbox.
2. **Tasks.** One row component at 20px indent: 6pt state dot (running blue, blocked
   red, needs-you yellow, otherwise a hairline ring), Lato 13/400 title, and the Session
   count as a right accessory only when > 0, in `textTertiary` with the bubble glyph.
   Row height 28. Hover shows the accessory tint; selection is the existing 2px burgundy
   edge plus tint, and keyboard highlight is a separate ring (Linear).
3. **Orphan Sessions.** Remove the "Repository or unavailable ancestry" subtitle and the
   two-line row. Put them in one conditional bottom section, **Other sessions**, caps 11
   header with a count, collapsed by default, rows one line (terminal glyph, name, age),
   appearing only while any exist. Sessions whose `workPath` matches a Task worktree fold
   under that Task (Zed, Codex). A hover action "Attach to task…" is the affordance
   that empties the section over time. The mock fixture must include this state so the
   two builds are compared honestly.

## 7. Session chrome

Jack: "weird to have similar tool sets on both the run and the session"; it "could
potentially get loud fast"; it is "visually disconnected from the left".

| Product | (a) Chrome per pane vs window; focus | (b) Identity | (c) Light ↔ dark join |
|---|---|---|---|
| Ghostty | None per split; `new_split`, `close_surface`, `toggle_split_zoom` are keybinds; unfocused splits dimmed 0.7 https://ghostty.org/docs/config/keybind/reference https://ghostty.org/docs/config/reference | Window title + native tabs only | `window-theme = auto`; `macos-titlebar-style = tabs` paints the titlebar in the terminal background |
| iTerm2 | Optional per-pane title bar; Maximize Active Pane in the Shell menu; corner maximize icon users asked to hide https://iterm2.com/documentation-preferences-appearance.html https://gitlab.com/gnachman/iterm2/-/issues/7288 | Window + tab + optional pane title | Minimal / Compact themes; "Show line under title bar… turn off for a sleek dark appearance" |
| Warp | Keys + right-click; triangle marks the active pane; "Dim inactive panes" toggle https://docs.warp.dev/terminal/appearance/pane-dimming/ | Identity lives in blocks and the vertical-tabs sidebar rows (title, cwd, branch, diff badge) https://docs.warp.dev/terminal/windows/vertical-tabs/ | One theme for the whole app |
| Zed | Active pane border 0, inactive opacity 1.0 by default https://zed.dev/docs/visual-customization | Terminal tabs; breadcrumb row removed from centre terminals as "wasted space" https://github.com/zed-industries/zed/issues/20475 | One theme |
| VS Code / Cursor | Split/kill are hover actions on the terminal tab list; ⌘\ splits https://code.visualstudio.com/docs/terminal/basics | Tab list hidden with a single terminal; its name moves into the panel header (`tabs.hideCondition: singleTerminal`) https://code.visualstudio.com/docs/terminal/appearance | `sideBar.background`, `panel.border`, `terminal.background` from one theme |
| Claude Code desktop | Every pane has a header for drag/pop-out; ⌘\ closes https://code.claude.com/docs/en/desktop | Sessions are sidebar tabs; terminal tabs inside the pane | Dark terminal in a light app, closed as not planned https://github.com/anthropics/claude-code/issues/45115 |
| Codex app | Terminal is one panel from a toolbar icon https://learn.chatgpt.com/codex/integrated-terminal | Panel shows branch and Local / Worktree / Cloud | Not documented |

What holds: split/close/zoom are keyboard and context-menu actions; nobody shows a
per-pane title by default; one identity slot at a time; focus is a dim or a small mark,
not a coloured frame; the seam is removed by one palette owning both sides.

**One chrome model for Loopflow.** Three bands become one:

- **Toolbar (the breadcrumb bar), always visible, in the sidebar tone `#F3EEE7` with a
  hairline below.** Left: `product / Show the all-wave… W2-131 / Release outcomes ✎`.
  Right: the worktree as a quiet mono chip (`loopflow.release-run`, folder glyph), the
  membership chip, then two icon buttons only: New pane (split menu: right / below /
  new shell) and Monitor. That is the entire always-visible chrome.
- **Pane header: none with one pane.** With ≥ 2 panes, a 24px strip inside the dark
  region, same as VS Code's single-terminal rule: name in mono 11, a 6pt state dot,
  and nothing else. Split / close / zoom live in the context menu, in keybinds
  (`⌘D`, `⌘⇧D`, `⌘W`, `⌘⇧↩`), and as a hover-only trio on the focused pane's strip
  that fades in over 100ms. Close on an unfocused pane never shows.
- **Focus:** unfocused panes at 0.85 opacity (Ghostty's mechanism, gentler than 0.7 so
  a companion `tail` stays readable). No blue border; the focused pane is the one at
  full opacity. Delete the floating "Complete" pill; Complete is a text button at the
  toolbar's right end, present only for a Session in a Flow step that accepts it.
- **Progressive-disclosure rule:** an action is visible at rest only if it is the
  single primary action of the surface (New session on Task, Complete on Session).
  Everything else is hover on the focused pane, context menu, keybind, or ⌘K.
- **Joining the dark region to the light sidebar:** the region is edge-to-edge under
  the toolbar (no 12px gap, no 12px radius; the mock's inset panes are the seam Jack
  sees). The terminal background shifts from cool `#2B3036` to a warm charcoal of the
  same temperature as cream (`#24211F`, chroma near cream's hue in LCH; verify against
  ANSI legibility); the toolbar, sidebar and terminal then share one hue family, which
  is how VS Code, Zed and Warp avoid the seam. Divider between panes is a 1px line at
  `#F0EBE4` 12% opacity. The sidebar's right hairline continues as the toolbar's bottom
  hairline so the L-shape reads as one piece (Linear's "inverted L").

## 8. Design baseline

Keeps cream, burgundy and the serif identity. Values are proposals; contrast must be
checked at 4.5:1 for text and 3:1 for chips before they land.

### Type ramp (Cormorant Garamond, Lato, JetBrains Mono)

| Step | Face | Size / line | Use |
|---|---|---|---|
| display | Cormorant 500 | 34 / 1.1 | Wave title (down from 46–48) |
| title | Cormorant 500 | 26 / 1.15 | Task title (down from 32) |
| lede | Cormorant 500 | 17 / 1.5, max 62ch | Wave objective; clamp to 4 lines with "more" |
| body | Lato 400 | 13 / 1.45 | Everything else |
| body-strong | Lato 700 | 13 | Row titles when selected, Session names |
| caption | Lato 400 | 11 / 1.3 | Meta, ranks, IDs |
| label | Lato 700 caps | 11, tracking 0.06em | Section headings, table heads |
| mono | JetBrains Mono | 12 / 1.5 | Skill names, IDs, chips, terminal chrome |

Seven steps, serif on three of them. Nothing between 17 and 26 exists on purpose.

### Spacing, radii, elevation

- 4pt grid: 4 / 8 / 12 / 16 / 24 / 32 / 48. Section gap 24; page padding 32 top, 40
  sides; page max 1160 (keep). Sidebar 240 (Zed default; 264 is more than the rows
  need once orphans and serif leave).
- Row heights: sidebar 28, plan 32, Session 40 (two lines).
- Radii: chips 4, buttons and inputs 6, rows 6, panels 10, terminal region 0.
- Elevation is a ladder, not shadows: canvas `#FAF8F5` → sidebar/toolbar `#F3EEE7` →
  panel `#FFFDF9` with hairline `#E6DFD6`. Shadows only on popovers and the palette:
  `0 8px 24px -12px rgba(42,38,36,.25)`. Remove the panel shadow in `workspacePanel`.

### Light palette (12-step warm ramp, OpenCode's role mapping, Linear's low-chroma rule)

| Step | Hex | Role |
|---|---|---|
| 1 | `#FAF8F5` | background (canvas) |
| 2 | `#F3EEE7` | backgroundPanel (sidebar, toolbar, notice base) |
| 3 | `#ECE6DD` | backgroundElement (hover, pressed, code inline) |
| 4 | `#FFFDF9` | surface (cards, plan panel, inputs) |
| 6 | `#E6DFD6` | borderSubtle (hairlines) |
| 7 | `#D6CCC0` | border (chips, inputs) |
| 8 | `#C2B5A6` | borderActive (focused input, hover outline) |
| 9 | `#722F37` | accent (burgundy: links, selection edge, primary outline, focus ring) |
| 10 | `#5E2630` | accentHover |
| 11 | `#7A716A` | textMuted (was `#8C837C`; darker for 4.5:1 on step 2) |
| 12 | `#2A2624` | text |

Terminal: background `#24211F`, foreground `#EDE7DF`, cursor and prompt accent
`#D9959D` (the existing rose `accentInk`), divider `rgba(240,235,228,.12)`. The
terminal belongs to the same hue family as the canvas; that is the whole seam fix.

State colours stay the current five, with one rule each: blue `#2F6BC0` means running
*and* loop region (loops are where running happens; no other blue anywhere, so the
breadcrumb membership chip becomes neutral mono); green `#3E7A4E` completed; yellow
`#9A6B12` needs Jack; red `#B23A30` blocked; grey `#6F6862` stopped. Fills stay opaque
(`WorkspaceTone.fill`) so hue survives inside a loop tint. Only one of these may
animate at a time: the running node and running status row carry a 2s shimmer
(Codex); nothing else moves at rest.

Dark: deferred; when it comes, derive the same 12 steps from a warm charcoal base with
the rose accent, not by opacity from light.

### Motion

`fast` 100ms ease-out for hover, chip and disclosure chevrons; `standard` 160ms
ease-in-out for panel open, row selection, pane focus dim; `panel` 240ms for sidebar
collapse and split creation; spring (0.3, 0.7) only for pane reorder. Reduce-motion
zeroes all of them; the shimmer becomes a static dot.

### Empty, loading, error

No spinner replaces content. Loading shows the last-good reading with the existing
"May be out of date" chip. Empty states are one line in caption plus the next action
(Things 3, DESIGN.md's affordance rule): "No sessions yet · New session". Errors are a
tinted line with a left rule, the reason, and one recovery action (Warp's failed block;
the Wave notice already does this).

### Keyboard

⌘K palette (Raycast rows, shortcuts right-aligned, recents first) over Waves, Tasks,
Sessions, Flows and actions; bottom search stays as the sidebar filter. In lists: ↑↓
highlight, ↩ open, `N` new session, `S` start/resume, `R` rename, `⌘D`/`⌘⇧D` split,
`⌘W` close pane, `⌘⇧↩` zoom, `Esc` up one breadcrumb level. Show them in the palette.

## 9. The ten highest-leverage changes to the native build, ranked

1. **Sidebar to one row component** (section 6): Waves 13/600 with chevron and no
   glyph; Tasks 13/400 with a 6pt dot only for running/blocked/needs-you; count
   accessory only when > 0; row 28. Fixes the incoherence Jack named first.
2. **Orphan Sessions into a conditional "Other sessions" section**, one-line rows,
   collapsed, folding matched worktrees under their Task; delete the "Repository or
   unavailable ancestry" subtitle. Also add this state to the mock fixture.
3. **Collapse the Session chrome to one toolbar** (section 7): delete
   `WorktreeNodeView`'s icon set and `SessionPaneView`'s header for a single pane;
   hover-only trio on the focused pane; Complete moves into the toolbar.
4. **Join the terminal to the frame**: toolbar in sidebar tone, terminal edge-to-edge,
   warm charcoal background, unfocused panes 0.85, no blue focus border, no 12px gaps
   or radii on panes.
5. **Compress the type ramp**: Wave 34, Task 26, objective 17 clamped to four lines,
   sidebar off serif. Same serif identity, one screen of hierarchy instead of two.
6. **Flow diagram legibility**: return edges land at distinct ports (Loop 1 top-left of
   `implement`, Loop 2 bottom-left), loop labels inside the region's top-left corner,
   connectors 1.5px at `border`, running node shimmer, `↳ then` becomes `↳ delivery`
   and queue/land carry their real state text.
7. **One status line for running work** under the Flow: `● review-slice · 12m · claude
   · esc to interrupt` in place of "Worker is running review-slice", with the elapsed
   time from the shared Run record (Codex/Claude Code pattern).
8. **Surface ladder instead of shadows**: remove the panel shadow, keep hairlines;
   `textMuted` darkened to `#7A716A`; single blue meaning (membership chip goes
   neutral mono).
9. **⌘K palette** over Waves/Tasks/Sessions/Flows/actions with visible shortcuts; list
   keys as in section 8. The bottom search field stays as a filter.
10. **Attention roll-up and push**: a yellow dot on a Wave row when a child needs Jack,
    an OS notification when a Session or Run finishes off-screen, and nothing that
    stays badged after it has been seen.

Not in the ten, worth recording: the installed build and the branch build differ (Wave
circles, orphan rows), so any acceptance capture must name which binary it came from;
and the mock's sample data should be regenerated from `lf` reads so it fails the same
way the app does.

# Research: workspace composition and missing affordances

Observed 2026-09-25. Research for the human-requested three-direction website study; no production UI decision is accepted yet.

## System understanding

Loopflow has one repo → Wave → Task → Session outline. Podium owns shared readings; navigation retains selection and presentation; the existing multiplexer retains Session, shell and Monitor panes. The product's distinct value is moving between intention and ongoing work without losing either. A generic document browser or chat sidebar cannot represent all of that alone.

The rejected configured capture is `demo-restore-evidence/configured-ready.png`. Earlier composition references include `main-view-round-two.png` and `ux-reference-images/loopflow-sessions-fixture.png`. The former uses cream chrome, a compact context header and a dominant charcoal workspace; the latter has a stronger burgundy application frame. These show historical design language, not evidence that either entire layout was accepted.

### External observations

**Notion.** Its documented sidebar supports nested disclosure, search, favorites, hiding and resizing. The official section-settings illustration visibly separates a pale navigation rail from a broad page; selection has a quiet filled row and the page has a distinct title. The illustration contains older example content and is not proof of every current pixel. Current documentation describes Home sections and top-level tabs that differ from that illustration. Borrow navigational legibility and bounded information; do not copy all its sections. [Sidebar documentation](https://www.notion.com/help/navigate-with-the-sidebar), [inspected official image](https://images.ctfassets.net/spoqsaf9291f/2tpJheiXLOwBH9ZdZGnQt3/514bd06e33972f166580fa5c50bcdbdf/Group_61.png).

**Figma.** Current documentation separates navigation, left panel, working canvas, right panel and toolbar. Labels are on by default; the left panel is resizable and the UI can minimize. Its official before/after image shows compact selected rows, whitespace between groups, and a persistent working surface. Its 2024 UI3 retrospective provides a counterexample to cosmetic simplification: moving clip content into a dropdown added a click, so the checkbox returned. That retrospective is historical, not the current interface specification. Borrow stable work space and contextual controls; copying Figma's full tool rail would add unnecessary Loopflow modes. [Current navigation](https://help.figma.com/hc/en-us/articles/360039831974-Explore-the-navigation-bar-and-left-sidebar), [inspected official image](https://help.figma.com/hc/article_attachments/41604884368023), [UI3 retrospective](https://www.figma.com/blog/our-approach-to-designing-ui3/).

**Claude Desktop.** Official documentation describes conversation beside an artifact or built-in browser, with results revisitable independently. This supports a focal conversation with related work nearby. Projects provide contained context; a new project experience is currently in staged beta, so it is unsafe to treat every subscriber's UI as identical. No authenticated desktop interaction was performed in this research; claims here are documented behavior, not a fresh pixel audit. Borrow contextual adjacency and a legible place to continue; avoid adding a Loopflow-owned transcript or turning every Task into a conversation. [Desktop walkthrough](https://academy.claude.com/tutorials/navigating-the-claude-desktop-app), [Artifacts](https://support.claude.com/en/articles/17153992-what-are-artifacts-and-how-do-i-use-them), [Projects and rollout limits](https://support.claude.com/en/articles/9517075-what-are-projects).

### General guidance

Progressive disclosure means making frequent actions available and secondary material discoverable on demand. It does not mean hiding everything behind an ellipsis. That distinction is supported by Nielsen Norman Group's [progressive-disclosure guidance](https://www.nngroup.com/articles/progressive-disclosure/) and Figma's checkbox counterexample. System status should tell people what happened and what is possible next; volume of diagnostic text is not a measure of clarity. [Visibility of system status](https://www.nngroup.com/articles/visibility-system-status/).

## Affordance audit

Observed source: `WorkspaceNavigator.swift`, `SessionsView.swift`, `WorkSurfaceView.swift`, `TaskMonitorView.swift` under `swift/LoopflowMac/Views/`. This is a targeted audit of the shown workspace, not a claim to have tested all application commands or accessibility.

| Affordance | Current evidence | Proposed study treatment |
|---|---|---|
| Clear primary working area | Configured view gives a dense rail and long inspector text much of the attention | Compare document-led, workspace-led and focus-led compositions using identical work |
| Readable navigation | Names and detail can each wrap to two lines; Session status competes horizontally | Strong row rhythm, one primary title, restrained secondary identity, full title on disclosure/help |
| Adjustable navigation | SessionsView gives WorkspaceNavigator a fixed 300-point width | Resize or retract it while retaining selected Work and drafts |
| Visible location | Task detail names its Wave and issue; Session display paths exist | Keep a short consistent location near the working surface, especially when hierarchy is collapsed |
| Selection versus focus | Code has exact selection and native focus ownership; color alone does not explain both | Separate selected Work treatment from active pane treatment; retain visible keyboard focus |
| Find and collapse | Already present, with one presentation menu | Preserve them; label the presentation choices and keep restoring the list obvious |
| Common actions | Inspect/New conversation also appear in context menus; actions elsewhere depend on selected view | Show the relevant Continue/Open action near the selected Work; leave infrequent actions secondary |
| Brief versus reference | Task description is displayed using plain Text; full condition precedes it | A bounded brief and formatted full directive on demand; no invented AI summary as authority |
| Actionable errors | Raw planning warnings precede all rows; Monitor already has Retry and partial-evidence disclosure | A concise scoped message plus Details/Retry; keep genuine uncertainty visible |
| No-Session state | Explicitly exists after readable evidence | Pair it with an understandable next action; distinguish upcoming, empty and unavailable |
| Return to work | Native retained panes and draft continuity have prior proofs | Give returning a visible affordance; mockup draft retention is only study behavior |
| Full keyboard operation | Some native shortcuts exist; complete tree-key navigation not established by this audit | Label controls, visible focus, Escape for dismissals; validate native traversal in implementation |

## Tensions

- Navigation breadth versus working space: upcoming Tasks must remain reachable even when a Session is the focus.
- Quiet versus discoverable: removing labels or burying Continue would reproduce the Figma counterexample.
- Truth versus readability: errors and unknown attribution must remain explicit, but raw recovery commands need not dominate every row.
- Brand versus saturation: burgundy, cream, serif headings and charcoal work areas are a vocabulary. A solid wine sidebar alone did not restore a good composition.
- Context versus ownership: inspecting another Task cannot silently relabel or move a retained conversation.

## Recommendations to test

**A — Work led.** Quiet outline, readable selected-Task page, brief and a clear continuation into its workspace. Strongest for orienting and reviewing intent; risk is delaying hands-on work.

**B — Workspace led.** Dominant retained terminal/conversation with companion and bounded contextual inspector. Strongest for long working sessions; risk is excessive tool density.

**C — Focus led.** One central Session/work area, retractable navigation and context disclosed on demand. Strongest for sustained conversation; risk is making other Work too remote.

All three must use the same sample Work and share Loopflow's visual vocabulary. They are composition proposals, not three different domain models. Keep the renderer native in any later implementation; the website is for design feedback only.

## Questions for the comparison

Where should attention land on opening a Task: its brief, its workspace, or its conversation? Which information should remain visible while typing? Which direction retains the recognizable Loopflow character? Can a person find another Task, return, inspect a problem and continue without guessing what will be replaced?

Do not call an option better from a screenshot alone. Try selection, details, search, collapse and draft return. The study should include a no-Session Task and explicit sample-data labeling. Dense data, failure states, keyboard traversal and native input are subsequent implementation proofs.

## Model execution boundary

The user explicitly requested Opus 5.5 to author the three mockups. The bounded launch selected `claude:claude-opus-5-5` through lf, but Claude Code 2.1.210 rejected it before generation, requiring 2.1.280 or newer. After the human updated Claude, the retry through `lf -m claude` started successfully. Its provider-authored message metadata reports `claude-opus-5-5`; the three mockups completed, with no substitution. The parent then corrected narrow-window sidebar recovery and inspector keyboard/visibility behavior; see `visual-study/receipt.json`. `visual-study/brief.md` is the prepared contribution brief. This research and the later integration/review remain the parent contribution.

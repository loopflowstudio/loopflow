# Backlog inspirations · LOO-903

Research read November 14, 2026. Official documentation and product material were inspected; the applications were not used live. Proposed Loopflow behavior below is an inference, not a claim about measured usability or a feature comparison based on trials.

## What seems useful

A library can be the repository's primary destination while a Task conversation remains the primary destination for doing that Task. The navigation target can determine the activity without permanently making planning or conversation win everywhere. This is a design recommendation for review.

Two independent decisions follow:
1. How much meaning is visible before opening a brief: dense rows, title plus excerpt, cards, or a sequence of open briefs.
2. Whether repository entry leads to its plan, restores an equal Plan/Conversation surface, or opens a composed workspace.

## Sources and mechanisms

| Reference | Documented mechanism | What it suggests for Loopflow | Tradeoff / what does not transfer |
|---|---|---|---|
| [Notion: database views](https://www.notion.com/help/views-filters-and-sorts) | One database can have several layouts and per-view filtering/grouping. Side peek leaves the collection interactive; full-page opening is also available. | Treat the brief as the content of a Task; let a scoped list remain visible during inspection. Separate view configuration from membership. | Too much view-building can make finding work require designing a database. Start with a few deliberate representations. |
| [Linear: display options](https://linear.app/docs/display-options), [custom views](https://linear.app/docs/custom-views), [favorites](https://linear.app/docs/favorites) | Group/order/display controls, Space peek on supported views, and personal sidebar shortcuts to views and records. | Dense rows for scanning, brief preview without navigating away, explicit shortcuts to important scopes. | Favorite-everything and issue-metadata density can obscure long briefs. Favorites are a reference, not a selected Loopflow feature. |
| [Asana: navigation](https://help.asana.com/s/article/navigating-asana) | Selecting a sidebar project/tag/person exposes the relevant task list; selecting a Task exposes its details pane. | Scope → collection → item is legible. Keep the scope header visible when details open. | A permanent scope/list/brief/conversation four-column composition needs too much width. A task details pane is not automatically a suitable terminal container. |
| [Obsidian: Bases](https://obsidian.md/help/bases), [views](https://obsidian.md/help/bases/views) | Multiple collection views over the underlying notes; a view can be embedded in another note. | A Wave objective can frame a live Task collection; a brief can stay readable as a document. Use existing shared planning, not Markdown as another store. | Obsidian's file storage does not transfer: Loopflow Task identity and lifecycle stay in the shared writer. Embedded lists must not become copied plans. |
| [Obsidian: tabs](https://obsidian.md/help/tabs), [workspaces](https://obsidian.md/help/plugins/workspaces), [bookmarks](https://obsidian.md/help/plugins/bookmarks) | Split/pinned/stacked tabs, saved layouts, and direct shortcuts to searches and headings. | Keep a library position while reading a chain of related briefs; retain a useful planning arrangement across switches. | General-purpose tab management would expand scope and duplicate existing pane owners. A short reading trail is worth sketching; unlimited stacked terminals are not implied. |
| [Scrivener: Binder, Corkboard and Outliner](https://www.literatureandlatte.com/blog/integrating-scriveners-binder-corkboard-and-outliner), [synopses](https://www.literatureandlatte.com/blog/10-things-you-can-do-with-synopses-in-your-scrivener-projects) | Different representations of the same manuscript pieces; Outliner/Corkboard let several synopses be seen together. | The backlog is also unfinished writing. A title plus meaningful excerpt can reveal scope and overlap before opening anything. Cards can help compare a small subset. | Do not add a separately maintained synopsis field on sight. Start from an excerpt of the canonical brief, visibly marked. A new authored summary would need its own design. |
| [Heptabase: UI logic](https://wiki.heptabase.com/user-interface-logic), [keyboard shortcuts](https://wiki.heptabase.com/keyboard-shortcuts) | A Card Library and contextual whiteboards share card data; cards report their locations. Removing a card's appearance differs from deleting it from the library. | A complete library and a focused working arrangement can coexist without duplicated Tasks. Keep current membership visible. | Spatial position is not planning priority. A canvas for the whole backlog would bury scan/order/search. Existing Dashboards own composition; the sketch does not introduce another canvas or card store. |
| [Things: scheduling](https://culturedcode.com/things/support/articles/2803579/) | Lists distinguish what is intended for today, actionable at any time, and uncertain future work. | Planning and doing deserve distinct destinations with clear language. | Those lists are not Loopflow execution states. Do not rename Working to Today, create a Someday status, or make selecting an item start it. |
| [DEVONthink: view menu, version 3.8.2](https://download.devontechnologies.com/download/devonthink/3.8.2/DEVONthink.help/Contents/Resources/pgs/menus-view.html), [getting started, version 3.8.2](https://download.devontechnologies.com/download/devonthink/3.8.2/DEVONthink.help/Contents/Resources/pgs/gettingstarted-simplified.html) | A navigation sidebar, reading list, group-specific sorting and smart groups give different access paths to the same library. | Distinguish durable membership from personal reading/selection context; remember sorting per scope. | These are historical versioned docs, not verification of the latest release. Its automatic smart rules are outside this proposal; view changes must not launch or mutate work. |

## What the familiar references leave open

Notion, Linear and Asana support collection/detail navigation. That does not determine which Loopflow destination should dominate: Loopflow also has live provider conversations and agent execution. Obsidian and Heptabase make the alternative visible—retain useful arrangements of content—but they do not prove that a narrow terminal beside a backlog is useful.

Scrivener supplies a representation that is closer to this Task's problem than a Kanban board: several pieces of unfinished prose can be judged together. The opening sketch tests excerpts, not a new summary model.

## Three application structures to try

**1 · Repository as library.** Clicking the repository name opens the scoped plan. Its conversation is a named secondary destination. Working Task shortcuts open their existing conversations directly. A compact list and persistent brief reader support rapid inspection. This gives planning priority on deliberate repository entry, while restoring an already-open Task window should retain its current work.

**2 · Plan and Conversation as peers.** A persistent repository header switches between equal Plan and Conversation surfaces. Repository entry restores the last surface. The plan can use rich excerpts and inline brief expansion. This avoids choosing one dominant activity, at the cost of a less predictable repository landing.

**3 · A composed reading workspace.** A pinned library shares a workspace with the repository conversation and an inspected brief; a short reading trail retains related briefs. Hide/show or focus a pane without replacing its owner. This is the radical candidate: inspect, compare and discuss with no lost context. It costs the most layout/retention work and can crowd a laptop display. On narrow layouts, explicitly switch surfaces rather than compressing four columns. No general canvas or new dashboard authority is proposed.

The page makes each structure navigable. Each can switch list representation independently, so choosing a navigation model does not select its default row style.

## Round 2 questions · historical

1. Does deliberate repository entry lead to **1a** the library, **1b** the last Plan/Conversation surface, or **1c** a composed workspace?
2. Which representation deserves the default: **2a** compact rows, **2b** excerpts, **2c** cards, **2d** reading trail? Other representations are exploratory, not a commitment to ship four modes.
3. Should the repository conversation be **3a** a separate full-width destination or **3b** an optional companion while planning? It remains the same ongoing conversation either way.

## Validation limits

Research is documentary. Reference visuals were not copied. Prototypes use an eight-Task sample with illustrative metadata; they prove no native input continuity, live query completeness or performance. Current repository/Wave layout already composes session/board/tiles through retained owners. Any chosen redesign must replace the relevant presentation while consuming those owners, preserve drafts and selection, and coordinate LOO-514 before changing broader Wave composition.

Round-2 DOM checks passed for the three navigation structures, four representations and retained sample drafts. Rendered browser validation remains unavailable in this Session; narrow and dark styles are authored but visually unverified.

The subsequent conversation supplied “1c, 2a, 3a”; the current combined direction and exact reply are recorded in [the design](loo-903-backlog-design.md#round-3--combined-direction). The current page combines a compact collection and retained brief reader with a separate full-width repository conversation.

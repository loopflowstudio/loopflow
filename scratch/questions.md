# LOO-488 — decisions and remaining judgment

2026-10-15 — Jack Heart approved implementation through demo review, then
requested stacking on LOO-487 / PR #1559 and redesigning the default and companion
palettes. The Task now records that parent; #1559 still lands first. Its parent
LOO-462 / #1531 retains the [Workflow-loading landing hold](workflow-loading.md).
No landing, installation or running-app restart follows from this work.

Accepted: independently assign any bundled/imported theme to Light and Dark;
Light/Dark/System Mode chooses the active assignment. Every theme supplies every
LoopflowPalette role; explicit aliases are allowed. Ghostty RGB imports convert
once to a complete persisted snapshot. Preview, Apply and Cancel remain app-wide.

Jack Heart requested two Light choices: keep **Loopflow Light** as the accepted
original cream/burgundy design and add **Neo Loopflow Light**. Implementation
choice: 25 offline choices (those two, redesigned Loopflow Dark, 22 companions).
Neo uses a quiet warm neutral canvas and restrained green action accent. The pack
spans multiple hue families; tentative green feedback did not request making all
themes green. Exact new colors remain demo judgment, not approved by test results.

Preserve the accepted filled Work selection geometry: no new edge stripe.
Choose independently calibrated app selection rather than terminal selection,
retaining 3:1 distinction and 4.5:1 selected text. Ordinary headings use quiet text;
focus/emphasized actions share a separate accent-ink role. Terminal renderer fills
belong to #1559, so delete the obsolete Swift paint fields and tests.

Remaining judgment: native coherence, live provider input/focus continuity,
comfortable sustained use and button-state appearance. Broader gate retains two
inherited WorkCacheTests findings. No blocking product choice remains.

Parent navigation, companion panes, copy/Find and contrast decisions are summarized
in [terminal polish](terminal-polish.md), with exact history references. LOO-416
owns relaunch layout persistence; no second layout store is introduced here.

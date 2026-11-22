# Questions

- Title bar: made a stated role (`loopflow-titlebar`) at kickoff instead of removing or lowering the 65% mix, because no mix keeps the other 11 dark bars' secondary text at 4.5:1 and gives Loopflow Dark a burgundy bar. Loopflow Dark's bar is `#722f37`, the same as Light's. Passed over: `#8a3a44` (Dark's button fill). Numbers in `8ae51f65b9:scratch/plan.md`.
- Concept review: the missing-bar fallback is today's color darkened until the 72% text reads, not "today's bar exactly"; the latter fails the plan's own test on fixture `747474/FFFFFF` (3.29:1). A personal theme with an unreadable derived bar gets a darker one after the update.
- Concept review: in a saved custom theme, changing "Button" no longer moves the title bar. Passed over: having the editor leave unstated roles out of the file so the bar keeps following.
- The title bar rule lightens a bar whose text is black (toward white), which the plan's "darkened" did not spell out; the READMEs say both.

- Round 2: `ThemeSample` is deleted rather than kept beside the cards. What it showed that no card does: a terminal line and the secondary text on the selected row.
- Round 2: the chosen card is a 2 pt ring in the window's focus color instead of the old selection fill and edge. Passed over: both.

- QA round 4: the inventory went stale a third time. Themes left the third-party inputs instead (the catalogue holds no third-party files now); a CI check for the rest is still not added, for the `Cargo.lock` reason.

- Concept review round 2: importing a dark file while in Light mode switches the preview to Dark mode rather than refusing. Passed over: switching to System (changes two sides), or filing it under dark Yours without previewing (the person sees nothing happen).
- Concept review round 2: the side buttons go; the chosen name moves into each section heading. Lost: in Light mode, seeing the dark side's name without switching to System.

- Round 3: in System mode an import previews the file on its side even when macOS is on the other side (as a card click does); Light/Dark mode switches the mode instead.
- Round 3: the Import tile's accessibility identifier is `appearance-import-theme` in both sections (the `-dark` variant went with the side).

- Concept review round 3: a card click takes its side from the theme's own colors, as import does, rather than from a side argument. Passed over: `previewTheme(name, side:)`, which keeps a parameter that can disagree with the theme.

- QA round 4: steer f4ce64ed ("make sure to make ipgone app respecy themes and the website he in loopflow light") arrived during QA. QA wrote it into the plan's Remaining rather than building two new surfaces in a QA step.

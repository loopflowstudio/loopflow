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

- Concept review round 4: `config.theme` drops `path` so the phone and Desktop ask the Machine the same question. Passed over: an optional `path` (two ways to ask).
- Concept review round 4: on the phone a single-name `theme:` is that theme whatever the phone's appearance, as on the Mac. Passed over: phone appearance picks, Loopflow default on the unnamed side (the steer summary's reading; Jack's words don't say it).
- Concept review round 4: the website terminal block becomes Loopflow Light's light terminal. Passed over: a dark block from Loopflow Dark.

- Implement round 5: Desktop no longer computes the config path; it shows what the server's `config.theme` answer names, so Advanced and the unknown-name message wait for the first answer. Passed over: keeping `ThemePaths.config` for display (a second source of the path).
- Implement round 5: the phone overrides `\.colorScheme` with the theme's own, so a single dark name draws system controls dark in light appearance. Sheets presented by UIKit may still follow the phone's appearance; check on the simulator.
- Implement round 5: the website's light terminal blocks and docs code take a 1 px `--border`, since Light's terminal background equals the page background and the blocks otherwise vanish. Passed over: `--surface-muted` behind them (no longer Light's terminal).
- Implement round 5: old `--bg` (white) maps to `--background` for the page and docs hero and `--surface` for cards, buttons and table cells; `--rule-strong` maps to `--border-active` (darker than before).

- QA round 5: `config.theme` lost `path` without a protocol revision bump, so a branch client and a 0.16.0 server refuse each other's request (both say revision 9). Left as is: the repo adds and reshapes operations without bumps, the phone falls back quietly and Desktop recovers when the server matches. Passed over: bumping to revision 10 (every new client would refuse every old server), or Desktop naming "update the server" for this error.

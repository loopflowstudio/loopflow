# Questions

- Title bar: made a stated role (`loopflow-titlebar`) at kickoff instead of removing or lowering the 65% mix, because no mix keeps the other 11 dark bars' secondary text at 4.5:1 and gives Loopflow Dark a burgundy bar. Loopflow Dark's bar is `#722f37`, the same as Light's. Passed over: `#8a3a44` (Dark's button fill). Numbers in `8ae51f65b9:scratch/plan.md`.
- Concept review: the missing-bar fallback is today's color darkened until the 72% text reads, not "today's bar exactly"; the latter fails the plan's own test on fixture `747474/FFFFFF` (3.29:1). A personal theme with an unreadable derived bar gets a darker one after the update.
- Concept review: in a saved custom theme, changing "Button" no longer moves the title bar. Passed over: having the editor leave unstated roles out of the file so the bar keeps following.
- The title bar rule lightens a bar whose text is black (toward white), which the plan's "darkened" did not spell out; the READMEs say both.

- Round 2: `ThemeSample` is deleted rather than kept beside the cards. What it showed that no card does: a terminal line and the secondary text on the selected row.
- Round 2: the chosen card is a 2 pt ring in the window's focus color instead of the old selection fill and edge. Passed over: both.
- Round 2: `ThemeDefinition.retired` has no removal date; it is the sentence a person with an old setting sees, not a file-shape fallback. No removal Task filed.

- QA round 2: the third-party inventory went stale a second time (a README edit after the refresh) and only the bundle build noticed. Not added: a `test_third_party.py` case calling `third_party.check()`, which would catch it in CI but also fail every PR that bumps `Cargo.lock` without a refresh.

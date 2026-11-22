# Questions

- Title bar: made a stated role (`loopflow-titlebar`) at kickoff instead of removing or lowering the 65% mix, because no mix keeps the other 11 dark bars' secondary text at 4.5:1 and gives Loopflow Dark a burgundy bar. Loopflow Dark's bar is `#722f37`, the same as Light's. Passed over: `#8a3a44` (Dark's button fill). Numbers in `scratch/plan.md`.
- Five light themes (Catppuccin Latte, Gruvbox Light, Flexoki Light, Linen, Noctis Lilac) get a title bar darker than their accent so secondary text reads. The alternative is leaving them at 3.35–4.35:1.
- Concept review: the missing-bar fallback is today's color darkened until the 72% text reads, not "today's bar exactly"; the latter fails the plan's own test on fixture `747474/FFFFFF` (3.29:1). A personal theme with an unreadable derived bar gets a darker one after the update.
- Concept review: in a saved custom theme, changing "Button" no longer moves the title bar. Passed over: having the editor leave unstated roles out of the file so the bar keeps following.
- Changed words in diffs: repaired by moving body text (and headings, which share it) in Catppuccin Latte and Solarized Dark, one value each. Solarized Dark's app text goes from `#9b9e9e` to `#b6b9b9`, lighter than Solarized's own and than its terminal text. Passed over: lowering the changed-word fill (`tone.ink.opacity(0.26)` in `ReviewStreamView`) for every theme, or a per-theme role for it.
- The title bar rule lightens a bar whose text is black (toward white), which the plan's "darkened" did not spell out; the READMEs say both.

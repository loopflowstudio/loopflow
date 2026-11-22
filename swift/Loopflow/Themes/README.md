# Bundled themes

`Catalogue/` holds the 24 fixed built-ins, 12 light and 12 dark. **Loopflow
Light** is the original cream/burgundy; **Loopflow Dark** puts the same burgundy
title bar over blue-grey, with rose headings and links; **Linen** and **Hearth**
are quiet light and charcoal alternatives.
Companions span amber, rose, violet, green, teal and blue actions with quiet text.

Every file is Ghostty theme syntax plus `loopflow-` app roles:

```
background = #fffcf0
foreground = #100f0f
palette = 4=#205ea6
loopflow-appearance = light
loopflow-accent = #847032
loopflow-titlebar = #685827
```

Built-ins must state every terminal color, ANSI 0–15 and every app role; your
theme files may state any subset. Missing app roles derive once through
`GhosttyThemeMapping`: ANSI 4 seeds action/focus and Working, 3 Waiting,
1 failure, 2 success. A missing `loopflow-titlebar` comes from the file's accent:
the accent itself in a light theme, the accent mixed into the background in a
dark one, then darkened until the title bar's secondary text reads at 4.5:1.
Colors are seeds, never activity. Missing ANSI colors come
from the matching Loopflow preset; 16–255 default to xterm's.

`Sources.md` records upstream terminal colors and marked adaptations;
`Licenses/` keeps each author's notice. Both SwiftPM and Xcode ship the
catalogue offline.

# Bundled themes

`Catalogue/` holds the 24 fixed built-ins, 12 light and 12 dark. **Loopflow
Light** is the original cream/burgundy; **Loopflow Dark** the original slate
with bronze; **Linen** and **Hearth** are quiet light and charcoal alternatives.
Companions span amber, rose, violet, green, teal and blue actions with quiet text.

Every file is Ghostty theme syntax plus `loopflow-` app roles:

```
background = #fffcf0
foreground = #100f0f
palette = 4=#205ea6
loopflow-appearance = light
loopflow-accent = #8a6f2e
```

Built-ins must state every terminal color, ANSI 0–15 and every app role; your
theme files may state any subset. Missing app roles derive once through
`GhosttyThemeMapping`: ANSI 4 seeds action/focus and Working, 3 Waiting,
1 failure, 2 success. Colors are seeds, never activity. Missing ANSI colors come
from the matching Loopflow preset; 16–255 default to xterm's.

`Sources.md` records upstream terminal colors and marked adaptations;
`Licenses/` keeps each author's notice. Both SwiftPM and Xcode ship the
catalogue offline.

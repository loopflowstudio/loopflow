# Bundled themes

`Catalogue/` holds 26 fixed built-ins: 13 families, each a light and a dark
theme. **Loopflow Light** is the original cream/burgundy; **Loopflow Dark** puts
the same burgundy title bar over blue-grey, with rose headings and links. The
other twelve families (Arcade, Contrast, Fig, Fjord, Folio, Graphite, Harbor,
Moss, Signal, Sorbet, Tannin, Terracotta) are Loopflow's own, written by
`scripts/theme_catalogue.py` from a short spec each:

```bash
uv run python scripts/theme_catalogue.py           # rewrite the 24 generated files and the prototype's table
uv run python scripts/theme_catalogue.py --check   # what a fresh run would change
```

Change a generated theme by editing its spec and rerunning; never edit its file.
A family's themes start with its name, which is how Choose Theme orders them.

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
dark one, then darkened (lightened, under black text) until the title bar's
secondary text reads at 4.5:1.
Colors are seeds, never activity. Missing ANSI colors come
from the matching Loopflow preset; 16–255 default to xterm's.

Both SwiftPM and Xcode ship the catalogue offline. A `theme:` setting that
names a built-in from before the families (Gruvbox Dark, Linen…) shows Loopflow
Light or Dark for that side and says so.

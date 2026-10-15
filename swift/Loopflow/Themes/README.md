# Bundled themes

Every JSON file supplies a complete Loopflow palette and terminal palette. Missing
roles fail decoding; unreadable roles fail validation. `Sources.md` records the
Ghostty inputs and marked cursor/selection adaptations. `LICENSE` and `Licenses/`
retain upstream notices. No original Ghostty file is a second runtime owner.

Loopflow Light retains the original design. Dark uses warm charcoal surfaces
and neutral selection, with burgundy action fills. Those colors belong to their records, not shared view defaults.

`GhosttyThemeMapping` converts imported terminal colors once: ANSI blue supplies
accent/Working, yellow Waiting, red failure and green success; selection colors
supply selected Work. Text, borders and surfaces derive from background/foreground.
State still determines which semantic role applies. Contrast adjusts the app
colors without changing imported terminal values.

The `LoopflowPalette` projection declares aliases explicitly: keyboard focus uses
emphasis; links/actions use text roles; status stroke/text use status ink. No
palette field has a decode default. Both SwiftPM and Xcode ship these files offline.

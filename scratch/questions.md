# LOO-488 — decisions and remaining judgment

October 15, 2026. Jack Heart redesigned theming around fixed built-ins, theme
files and Ghostty-syntax `theme:` config. The [whole-app plan](whole-app-theming.md)
owns current implementation; the parent [Workflow-loading landing hold](workflow-loading.md)
survives. No landing, install or replacement of running clients is authorized.

Reversible implementation choices (theme-file cut), for Jack's review:

- Built-ins ship in the same Ghostty-syntax format as your files (one parser);
  bundled JSON is gone. App-role keys are `loopflow-<role>` (kebab-case).
- Customize, edits, Delete and Import are staged until Apply, so Cancel writes
  nothing anywhere. Imports are written in full normalized form, not as the
  original bytes.
- Ghostty-folder themes appear under Yours, read-only: Customize copies them into
  `~/.lf/themes`; no Delete.
- The editor no longer renames or flips appearance: the filename is the name.
- A hand-edited pair that puts a dark theme on the light side is accepted as
  written; only unknown names are refused.
- New companion app palettes come from the import mapping with one authored
  accent family each (Manila/Rosewood rose, Clay/Flexoki amber, Noctis/Aubergine
  violet, Pewter/Frost/Night Owl blue, Celadon/Onyx teal, Vellum/Moss/Phosphor
  green) and a dark shadow. Exact colors remain review judgment.
- Outside edits reload through a one-second fingerprint poll of the config and
  both theme folders.

Native coherence, editor usability and live provider input/focus continuity remain
open. Parent renderer/navigation/pane behavior remains owned by #1559; LOO-416
owns relaunch layout persistence.

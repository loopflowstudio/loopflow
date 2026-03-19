# Open questions / assumptions

- Implemented the March 19 pane-routing spec as the source of truth for the multiplexer draft: exactly one outer terminal pane per wave, with tmux handling shell splits/windows inside it.
- Kept the existing shortcut surface (`splitVertical`, `splitHorizontal`, `closePane`, `newShellPane`, `focusNextPane`, `focusPreviousPane`) and made it context-sensitive instead of renaming actions to the directional tmux vocabulary from the design note.
- When a non-terminal outer pane is split, the new pane type cycles through native panes (`markdown` → `diff` → `launchpad`) rather than prompting the user.

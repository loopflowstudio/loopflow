# Update Hero GIF

Replace `docs/demo.gif` with a recording that demonstrates the actual workflow from the README.

## What to build

A new `demo.tape` showing two scenes: (1) debug an error, (2) build a feature end-to-end. The GIF should mirror the README Quick Start exactly.

## Files to change

```
demo.tape           # Update VHS script
docs/demo.gif       # Regenerated output (via vhs)
```

## New demo.tape

```tape
# VHS Demo for Loopflow
Output docs/demo.gif

Set FontSize 14
Set Width 900
Set Height 500
Set Theme "Catppuccin Mocha"
Set Padding 20

# Scene 1: Debug
Type "# Debug an error"
Enter
Sleep 1s

Type "# Copy error to clipboard, then:"
Enter
Sleep 500ms

Type "lf debug -v"
Enter
Sleep 3s

# Scene 2: Build a feature
Type ""
Enter
Type "# Build a feature"
Enter
Sleep 1s

Type "wt switch --create my-feature"
Enter
Sleep 1.5s

Type "lf design: add user authentication"
Enter
Sleep 2s

Type "lf implement && lf polish && lf review"
Enter
Sleep 2s

Type "lfops pr"
Enter
Sleep 2s

Type "# Done!"
Enter
Sleep 1.5s
```

## Constraints

- **No `-i` flag on design**: Design is interactive by default. Matches README style.
- **VHS types only, no execution**: The demo shows the workflow, not real output.
- **Keep short**: <20s total runtime for the hero GIF.
- **Match README exactly**: Commands must mirror the Quick Start section.

## UI changes

None. This is documentation-only (demo GIF and VHS script).

## Done when

1. `demo.tape` replaced with new content
2. Run: `vhs demo.tape` (from repo root, with venv activated)
3. `docs/demo.gif` shows debug + feature workflow
4. Commands in GIF match README Quick Start exactly

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

# VHS Demo for Loopflow
Output docs/demo.gif

Set FontSize 14
Set Width 900
Set Height 500
Set Theme "Catppuccin Mocha"
Set Padding 20
Set TypingSpeed 50ms

# Scene 1: Debug
Type "# Debug an error"
Enter
Sleep 400ms

Type "# Copy error to clipboard, then:"
Enter
Sleep 300ms

Type "lf debug -v"
Enter
Sleep 1s

# Scene 2: Build a feature
Type ""
Enter
Type "# Build a feature"
Enter
Sleep 400ms

Type "wt switch --create my-feature"
Enter
Sleep 600ms

Type "lf design: add user authentication"
Enter
Sleep 800ms

Type "lf implement && lf polish && lf review"
Enter
Sleep 800ms

Type "lfops pr"
Enter
Sleep 600ms

Type "# Done!"
Enter
Sleep 800ms

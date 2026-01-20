# VHS Demo for Loopflow
Output docs/demo.gif

Set FontSize 14
Set Width 900
Set Height 500
Set Theme "Catppuccin Mocha"
Set Padding 20
Set Shell "bash"

# Create stub commands that do nothing (for clean demo output)
Hide
Type "mkdir -p /tmp/demo-bin && for c in lf lfops; do echo '#!/bin/bash' > /tmp/demo-bin/$c && chmod +x /tmp/demo-bin/$c; done"
Enter
Sleep 500ms
Type "export PATH=/tmp/demo-bin:$PATH"
Enter
Sleep 500ms
Type "clear"
Enter
Sleep 200ms
Show

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

Type "lfops wt create my-feature"
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

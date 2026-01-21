# VHS Demo: Loops & lfd
Output docs/loops-demo.gif

Set FontSize 14
Set Width 900
Set Height 500
Set Theme "Catppuccin Mocha"
Set Padding 20
Set Shell "bash"

# Create stub commands
Hide
Type "mkdir -p /tmp/demo-bin && for c in lfd; do echo '#!/bin/bash' > /tmp/demo-bin/$c && chmod +x /tmp/demo-bin/$c; done"
Enter
Sleep 500ms
Type "export PATH=/tmp/demo-bin:$PATH"
Enter
Sleep 500ms
Type "clear"
Enter
Sleep 200ms
Show

# Scene 1: Show available goals
Type "# See what goals are available"
Enter
Sleep 1s

Type "lfd list-goals"
Enter
Sleep 2s

# Scene 2: Start a loop
Type ""
Enter
Type "# Start a continuous loop"
Enter
Sleep 1s

Type "lfd loop Maestro/ --flow ship"
Enter
Sleep 2s

# Scene 3: Check status
Type ""
Enter
Type "# Check loop status"
Enter
Sleep 1s

Type "lfd status"
Enter
Sleep 2s

# Scene 4: Run a one-shot flow
Type ""
Enter
Type "# Or run a single iteration"
Enter
Sleep 1s

Type "lfd flow Maestro/ --flow ship -p spec.md"
Enter
Sleep 2s

# Scene 5: See PRs
Type ""
Enter
Type "# See created PRs"
Enter
Sleep 1s

Type "lfd prs product-engineer"
Enter
Sleep 2s

Type "# Loops run continuously in background"
Enter
Sleep 1.5s

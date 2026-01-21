# VHS Demo: Subscribe & Schedule Triggers
Output docs/triggers-demo.gif

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

# Scene 1: Subscribe to file changes
Type "# Watch files and react to changes"
Enter
Sleep 1s

Type "lfd subscribe src/api/ 'src/api/**' --flow ship"
Enter
Sleep 2s

# Scene 2: Schedule a daily run
Type ""
Enter
Type "# Schedule to run at 9am daily"
Enter
Sleep 1s

Type "lfd schedule . '0 9 * * *' --flow ship"
Enter
Sleep 2s

# Scene 3: Check all triggers
Type ""
Enter
Type "# See all loops and triggers"
Enter
Sleep 1s

Type "lfd status"
Enter
Sleep 2.5s

# Scene 4: Grace period explanation
Type ""
Enter
Type "# Schedules have 24h grace for laptops:"
Enter
Sleep 1s

Type "# Missed 9am? Still runs when you wake at 2pm"
Enter
Sleep 2s

# Scene 5: Multiple triggers
Type ""
Enter
Type "# Combine triggers as needed"
Enter
Sleep 1s

Type "lfd subscribe . 'schema.graphql' --flow ship"
Enter
Sleep 1.5s

Type "lfd schedule . '0 10 * * MON' --flow ship"
Enter
Sleep 1.5s

Type "# Reactive agents, working in the background"
Enter
Sleep 1.5s

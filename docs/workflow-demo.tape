# VHS Demo: Feature Workflow (Tier 2)
Output docs/workflow-demo.gif

Set FontSize 14
Set Width 900
Set Height 500
Set Theme "Catppuccin Mocha"
Set Padding 20
Set Shell "bash"

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

Type "lfops wt create add-divide"
Enter
Sleep 1.5s

Type "lf design: add division to calculator"
Enter
Sleep 2s

Type "lf implement && lf polish"
Enter
Sleep 2s

Type "lfops pr"
Enter
Sleep 2s

Type "# Feature shipped!"
Enter
Sleep 1.5s

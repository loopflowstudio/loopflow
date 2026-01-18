# VHS Demo: Quick Debug
Output docs/debug-demo.gif

Set FontSize 14
Set Width 900
Set Height 500
Set Theme "Catppuccin Mocha"
Set Padding 20
Set Shell "bash"

Hide
Type "mkdir -p /tmp/demo-bin && echo '#!/bin/bash' > /tmp/demo-bin/lf && chmod +x /tmp/demo-bin/lf"
Enter
Sleep 500ms
Type "export PATH=/tmp/demo-bin:$PATH"
Enter
Sleep 500ms
Type "clear"
Enter
Sleep 200ms
Show

Type "# Run tests, copy the error to clipboard"
Enter
Sleep 1s

Type "lf debug -v"
Enter
Sleep 2s

Type "# Fixed!"
Enter
Sleep 1.5s

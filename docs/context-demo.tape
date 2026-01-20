# VHS Demo: Context Assembly
Output docs/context-demo.gif

Set FontSize 14
Set Width 900
Set Height 550
Set Theme "Catppuccin Mocha"
Set Padding 20
Set Shell "bash"

Hide
Type "mkdir -p /tmp/demo-bin && cp docs/stubs/lf-context /tmp/demo-bin/lf"
Enter
Sleep 100ms
Type "export PATH=/tmp/demo-bin:$PATH"
Enter
Sleep 100ms
Type "clear"
Enter
Sleep 200ms
Show

Type "# What context does loopflow assemble?"
Enter
Sleep 1s

Type "lf design"
Enter
Sleep 4s

Type ""
Enter
Type "# docs, branch files, summaries, clipboard - all automatic"
Enter
Sleep 2s

Type "# Configure in .lf/config.yaml"
Enter
Sleep 1.5s

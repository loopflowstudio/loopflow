# VHS Demo: Context Assembly
Output docs/context-demo.gif

Set FontSize 14
Set Width 900
Set Height 600
Set Theme "Catppuccin Mocha"
Set Padding 20
Set Shell "bash"

# Create stub lf that shows realistic output
Hide
Type "mkdir -p /tmp/demo-bin"
Enter
Sleep 100ms
Type @"cat > /tmp/demo-bin/lf << 'SCRIPT'
#!/bin/bash
echo ""
echo "Tokens: 22,932"
echo ""
echo "files          17,625 ███████████████"
echo "  docs         13,113 ███████████"
echo "    config.md   1,336 █"
echo "    lf.md         969 ▏"
echo "    patterns.md   847 ▏"
echo "    (7 more)    9,961 ████████"
echo "  STYLE.md      2,158 █"
echo "  .design       1,239 █"
echo "  README.md       504 ▏"
echo "summary         4,016 ███"
echo "  src/backend   2,500 ██"
echo "  src/frontend  1,516 █"
echo "loopflow          657 ▏"
echo "task              634 ▏"
echo ""
echo "Copied to clipboard."
SCRIPT"@
Enter
Sleep 100ms
Type "chmod +x /tmp/demo-bin/lf"
Enter
Sleep 100ms
Type "export PATH=/tmp/demo-bin:$PATH"
Enter
Sleep 100ms
Type "clear"
Enter
Sleep 200ms
Show

Type "# See what context loopflow assembles"
Enter
Sleep 1s

Type "lf implement -c"
Enter
Sleep 4s

Type ""
Enter
Type "# Docs, summaries, design specs - assembled automatically"
Enter
Sleep 2s

Type "# Configure in .lf/config.yaml:"
Enter
Sleep 500ms

Type "cat .lf/config.yaml"
Enter
Sleep 100ms

Hide
Type @"echo '
summary_tokens: 25000
summaries:
  - path: src/backend
  - path: src/frontend

context:
  - src/schema.py
  - docs/api.md
'"@
Enter
Show
Sleep 3s

Type ""
Enter
Type "# No more 'context lost' - you control what goes in"
Enter
Sleep 2s

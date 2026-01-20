# VHS Demo: Quick Debug (Tier 1)
# Shows: context assembly → Claude Code interactive session
Output docs/debug-demo.gif

Set FontSize 14
Set Width 900
Set Height 600
Set Theme "Catppuccin Mocha"
Set Padding 20
Set Shell "bash"

# Create stub that shows context assembly then "launches" Claude
Hide
Type "mkdir -p /tmp/demo-bin"
Enter
Sleep 100ms
Type @"cat > /tmp/demo-bin/lf << 'SCRIPT'
#!/bin/bash
echo ""
echo "Tokens: 8,247"
echo ""
echo "files          4,892 ████"
echo "  STYLE.md     1,854 █"
echo "  README.md      588 ▏"
echo "diff_files     2,721 ██"
echo "  src/calc.py  1,847 █"
echo "  tests/       874 ▏"
echo "clipboard        634 ▏"
echo ""
echo "Launching Claude Code..."
echo ""
sleep 1
echo "╭─────────────────────────────────────────────────────────╮"
echo "│  Claude Code                                            │"
echo "╰─────────────────────────────────────────────────────────╯"
echo ""
echo "I can see the error from your clipboard:"
echo ""
echo "  AssertionError: expected 4, got 5"
echo "  in test_add at tests/test_calc.py:12"
echo ""
echo "Looking at src/calc.py..."
echo ""
sleep 1
echo "Found the bug - there's an off-by-one error on line 8."
echo "Fixing it now."
echo ""
SCRIPT"
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

Type "# Copy an error to clipboard, then:"
Enter
Sleep 1s

Type "lf debug -v"
Enter
Sleep 6s

Type ""
Enter
Type "# Normal Claude Code session - context preloaded"
Enter
Sleep 2s

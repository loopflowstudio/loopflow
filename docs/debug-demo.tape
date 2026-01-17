# VHS Demo: Quick Debug with loopflow
# Shows: copy error → lf debug -v → fixed

Output docs/debug-demo.gif

Set FontSize 14
Set Width 900
Set Height 500
Set Theme "Catppuccin Mocha"
Set Padding 20

# Start in the demo repo
Type "cd loopflow-demos/calculator"
Enter
Sleep 500ms

# Run tests to see the failure
Type "python -m pytest test_calc.py"
Enter
Sleep 3s

# Copy the error (simulated - in real demo, user copies to clipboard)
Type "# Copy the error to clipboard, then..."
Enter
Sleep 1s

# Run debug with clipboard paste
Type "lf debug -v"
Enter
Sleep 15s

# Run tests again to show it's fixed
Type "python -m pytest test_calc.py"
Enter
Sleep 3s

Type "# Fixed!"
Enter
Sleep 2s

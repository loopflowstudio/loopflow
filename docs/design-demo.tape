# VHS Demo: Full Workflow with loopflow
# Shows: design → implement → polish → review

Output docs/design-demo.gif

Set FontSize 14
Set Width 900
Set Height 500
Set Theme "Catppuccin Mocha"
Set Padding 20

# Start in the demo repo
Type "cd loopflow-demos/calculator"
Enter
Sleep 500ms

# Create a feature branch
Type "wt switch --create add-divide"
Enter
Sleep 2s

# Design the feature (interactive session - abbreviated)
Type "lf design: add division to the calculator"
Enter
Sleep 10s

# Implementation pipeline
Type "lf implement && lf polish && lf review"
Enter
Sleep 20s

# Show the result
Type "cat calc.py"
Enter
Sleep 3s

Type "python -m pytest test_calc.py"
Enter
Sleep 3s

Type "# Ready to ship!"
Enter
Sleep 2s

#!/bin/bash
set -e

echo "=== Done When Test ==="
echo

# Clean up any previous maestro state
echo "Cleaning up..."
uv run lf maestro stop 2>/dev/null || true
rm -f ~/.lf/maestro.*

# Start maestro
echo "$ lf maestro start"
uv run lf maestro start
echo

# Verify socket created
if [ ! -S ~/.lf/maestro.sock ]; then
    echo "✗ maestro.sock not created"
    exit 1
fi
echo "✓ Maestro listening on ~/.lf/maestro.sock"
echo

sleep 1

# Check initial status
echo "$ lf status"
uv run lf status
echo

# Create test task
echo "Creating test task..."
cat > .lf/quick_task.lf <<'EOF'
Print "hello from maestro test"
EOF

# Run task in print mode (background)
echo "$ lf quick_task -p &"
uv run lf quick_task -p > /tmp/task_output.txt 2>&1 &
TASK_PID=$!

# Give it time to register
sleep 2

# Check status while running
echo "$ lf status"
uv run lf status
echo

# Wait for task to complete
wait $TASK_PID || true

# Give maestro time to update
sleep 1

# Check final status
echo "$ lf status"
uv run lf status
echo

# Check task output
echo "Task output:"
cat /tmp/task_output.txt | head -5
echo

# Stop maestro
echo "$ lf maestro stop"
uv run lf maestro stop
echo

echo "=== Test Complete ==="

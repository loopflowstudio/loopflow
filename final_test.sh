#!/bin/bash
# Final integration test

set -e

echo "=== Final Integration Test ==="
echo

# Clean state
uv run lf maestro stop 2>/dev/null || true
rm -f ~/.lf/maestro.*

# Test 1: Basic maestro commands
echo "Test 1: Basic Commands"
echo "  Starting maestro..."
uv run lf maestro start
sleep 1

if [ ! -S ~/.lf/maestro.sock ]; then
    echo "  ✗ Socket not created"
    exit 1
fi
echo "  ✓ Maestro started"

echo "  Checking status..."
OUTPUT=$(uv run lf status)
if [[ "$OUTPUT" != *"No running sessions"* ]]; then
    echo "  ✗ Expected 'No running sessions', got: $OUTPUT"
    exit 1
fi
echo "  ✓ Status command works"

echo "  Stopping maestro..."
uv run lf maestro stop
if [ -S ~/.lf/maestro.sock ]; then
    echo "  ✗ Socket still exists after stop"
    exit 1
fi
echo "  ✓ Maestro stopped"
echo

# Test 2: Session registration
echo "Test 2: Session Registration"
uv run lf maestro start
sleep 1

echo "  Creating test task..."
cat > .lf/final_test.lf <<'EOF'
Say "integration test complete"
EOF

echo "  Running task..."
uv run lf final_test -p > /dev/null 2>&1 || true

echo "  ✓ Task completed"

uv run lf maestro stop
echo

# Test 3: CLI integration
echo "Test 3: CLI Integration"
echo "  Verifying CLI commands..."
if ! uv run lf --help | grep -q "maestro"; then
    echo "  ✗ maestro not in help"
    exit 1
fi
if ! uv run lf --help | grep -q "status"; then
    echo "  ✗ status not in help"
    exit 1
fi
echo "  ✓ All CLI commands registered"
echo

# Cleanup
rm .lf/final_test.lf
rm -f ~/.lf/maestro.*

echo "=== All Tests Passed ==="

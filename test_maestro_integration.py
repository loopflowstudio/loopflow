#!/usr/bin/env python3
"""Test maestro integration with task execution."""

import subprocess
import time
from pathlib import Path

def test_maestro_integration():
    """Test that tasks register with maestro."""

    # Ensure maestro is running
    result = subprocess.run(
        ["uv", "run", "lf", "maestro", "start"],
        capture_output=True,
        text=True,
    )
    print(f"Start maestro: {result.stdout.strip()}")

    # Check initial status
    result = subprocess.run(
        ["uv", "run", "lf", "status"],
        capture_output=True,
        text=True,
    )
    print(f"Initial status: {result.stdout.strip()}")
    assert "No running sessions" in result.stdout

    # Create a simple test task
    test_dir = Path(".lf")
    test_dir.mkdir(exist_ok=True)
    (test_dir / "quick_test.lf").write_text("Say hello")

    # Start a task in background (simulating print mode)
    print("\nStarting task in background...")
    proc = subprocess.Popen(
        ["uv", "run", "lf", "quick_test", "-p"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    # Give it a moment to register
    time.sleep(2)

    # Check status while running
    result = subprocess.run(
        ["uv", "run", "lf", "status"],
        capture_output=True,
        text=True,
    )
    print(f"\nStatus while running:\n{result.stdout}")

    # Verify session is registered
    if "quick_test" in result.stdout and "running" in result.stdout:
        print("✓ Task registered with maestro")
    else:
        print("✗ Task NOT registered with maestro")
        print(f"Status output: {result.stdout}")

    # Wait for task to complete
    print("\nWaiting for task to complete...")
    proc.wait(timeout=30)

    # Give maestro time to update
    time.sleep(1)

    # Check final status
    result = subprocess.run(
        ["uv", "run", "lf", "status"],
        capture_output=True,
        text=True,
    )
    print(f"\nFinal status:\n{result.stdout}")

    # Stop maestro
    result = subprocess.run(
        ["uv", "run", "lf", "maestro", "stop"],
        capture_output=True,
        text=True,
    )
    print(f"\nStop maestro: {result.stdout.strip()}")

    print("\n✓ Test complete")

if __name__ == "__main__":
    test_maestro_integration()

#!/usr/bin/env python3
"""Test CLI maestro integration."""

import subprocess
import sys
import time

def main():
    # Start maestro
    print("Starting maestro...")
    subprocess.run(["uv", "run", "lf", "maestro", "start"], check=True)
    time.sleep(1)

    # Check status (should be empty)
    result = subprocess.run(
        ["uv", "run", "lf", "status"],
        capture_output=True,
        text=True,
    )
    print(f"Initial status:\n{result.stdout}")

    # Create a test task
    print("\nCreating test task...")
    with open(".lf/sleep_test.lf", "w") as f:
        f.write("Sleep for a moment")

    # Start a task in the background
    print("Starting task in background...")
    proc = subprocess.Popen(
        ["uv", "run", "lf", "sleep_test", "-p"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    # Give it time to register
    time.sleep(3)

    # Check status while task is running
    result = subprocess.run(
        ["uv", "run", "lf", "status"],
        capture_output=True,
        text=True,
    )
    print(f"\nStatus while running:\n{result.stdout}")

    if "sleep_test" in result.stdout:
        print("✓ Task is registered!")
    else:
        print("✗ Task not found in status")
        # Print task output for debugging
        proc.wait(timeout=5)
        print(f"\nTask stdout:\n{proc.stdout.read().decode()}")
        print(f"\nTask stderr:\n{proc.stderr.read().decode()}")
        sys.exit(1)

    # Wait for task to finish
    proc.wait(timeout=30)

    # Check final status
    time.sleep(1)
    result = subprocess.run(
        ["uv", "run", "lf", "status"],
        capture_output=True,
        text=True,
    )
    print(f"\nFinal status:\n{result.stdout}")

    # Stop maestro
    print("\nStopping maestro...")
    subprocess.run(["uv", "run", "lf", "maestro", "stop"], check=True)

    print("\n✓ All tests passed!")

if __name__ == "__main__":
    main()

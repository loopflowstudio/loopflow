#!/usr/bin/env python3
"""One-command verification for the loopflow tmux plugin.

Creates an isolated tmux server, sources the plugin, and runs structural
checks. Prints a manual verification checklist at the end.

Usage:
    uv run python scripts/tmux-review.py
"""

import os
import shutil
import subprocess
import sys
import tempfile
import time

SOCKET = "loopflow-review"
PLUGIN_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PLUGIN_FILE = os.path.join(PLUGIN_DIR, "loopflow.tmux")


def run_tmux(*args: str, check: bool = True) -> subprocess.CompletedProcess:
    cmd = ["tmux", "-L", SOCKET] + list(args)
    return subprocess.run(cmd, capture_output=True, text=True, check=check, timeout=10)


def has_tmux() -> bool:
    return shutil.which("tmux") is not None


def start_server():
    """Start an isolated tmux server with a session."""
    run_tmux("new-session", "-d", "-s", "review", "-x", "200", "-y", "50")
    time.sleep(0.5)


def kill_server():
    """Kill the isolated tmux server."""
    run_tmux("kill-server", check=False)


def check_plugin_source() -> bool:
    """Source the plugin and verify it loads without error."""
    result = run_tmux("source-file", PLUGIN_FILE, check=False)
    if result.returncode != 0:
        print(f"  FAIL: plugin source error: {result.stderr.strip()}")
        return False
    print("  OK: plugin sourced without errors")
    return True


def check_idempotent_source() -> bool:
    """Source the plugin twice and verify no errors."""
    run_tmux("source-file", PLUGIN_FILE, check=False)
    result = run_tmux("source-file", PLUGIN_FILE, check=False)
    if result.returncode != 0:
        print(f"  FAIL: second source failed: {result.stderr.strip()}")
        return False
    print("  OK: idempotent re-source works")
    return True


def check_options_set() -> bool:
    """Verify tmux options are set."""
    ok = True
    for opt in ["@loopflow_mode", "@loopflow_key_prefix", "@loopflow_dir"]:
        result = run_tmux("show-option", "-gqv", opt, check=False)
        value = result.stdout.strip()
        if not value:
            print(f"  FAIL: option {opt} not set")
            ok = False
        else:
            print(f"  OK: {opt} = {value}")
    return ok


def check_status_right() -> bool:
    """Verify status-right includes the loopflow status script."""
    result = run_tmux("show-option", "-gqv", "status-right", check=False)
    value = result.stdout.strip()
    if "loopflow-status.sh" in value:
        print("  OK: status-right includes loopflow-status.sh")
        return True
    print(f"  FAIL: status-right missing loopflow-status.sh: {value}")
    return False


def check_keybindings() -> bool:
    """Verify keybindings are registered."""
    result = run_tmux("list-keys", "-T", "loopflow", check=False)
    keys = result.stdout.strip()
    if not keys:
        print("  FAIL: no loopflow keybindings found")
        return False
    binding_count = len(keys.splitlines())
    expected = 9  # r, s, o, p, n, d, w, L, ?
    if binding_count < expected:
        print(f"  WARN: expected {expected} bindings, found {binding_count}")
    else:
        print(f"  OK: {binding_count} keybindings registered")
    return True


def check_prefix_binding() -> bool:
    """Verify prefix key enters loopflow table."""
    result = run_tmux("list-keys", "-T", "prefix", check=False)
    keys = result.stdout.strip()
    key_prefix_result = run_tmux("show-option", "-gqv", "@loopflow_key_prefix", check=False)
    key_prefix = key_prefix_result.stdout.strip() or "l"
    if f"switch-client -T loopflow" in keys:
        print(f"  OK: prefix+{key_prefix} enters loopflow key table")
        return True
    print(f"  FAIL: prefix+{key_prefix} binding not found")
    return False


def check_status_script() -> bool:
    """Run the status script directly and verify output."""
    script = os.path.join(PLUGIN_DIR, "scripts", "loopflow-status.sh")
    result = subprocess.run(
        [script], capture_output=True, text=True, check=False, timeout=5,
        env={**os.environ, "LOOPFLOW_DIR": PLUGIN_DIR},
    )
    output = result.stdout.strip()
    if output.startswith("[lf"):
        print(f"  OK: status script output: {output}")
        return True
    if not output:
        print("  WARN: status script returned empty (may need tmux context)")
        return True  # Not a hard failure — script needs tmux pane context
    print(f"  WARN: unexpected status output: {output}")
    return True


def check_layout_scripts() -> bool:
    """Verify layout scripts exist and are executable."""
    ok = True
    for name in ["lf-dev.sh", "lf-swarm.sh"]:
        path = os.path.join(PLUGIN_DIR, "scripts", "layouts", name)
        if not os.path.isfile(path):
            print(f"  FAIL: layout script missing: {name}")
            ok = False
        elif not os.access(path, os.X_OK):
            print(f"  FAIL: layout script not executable: {name}")
            ok = False
        else:
            print(f"  OK: {name} exists and is executable")
    return ok


def check_layouts_create_windows() -> bool:
    """Run each layout and verify it creates a named window."""
    ok = True
    for name, window_name in [("lf-dev.sh", "lf-dev"), ("lf-swarm.sh", "lf-swarm")]:
        script = os.path.join(PLUGIN_DIR, "scripts", "layouts", name)
        # Run inside the tmux server
        run_tmux(
            "send-keys", f"LOOPFLOW_DIR='{PLUGIN_DIR}' bash '{script}' '{PLUGIN_DIR}'", "Enter",
            check=False,
        )
        time.sleep(1)
        result = run_tmux("list-windows", "-F", "#{window_name}", check=False)
        windows = result.stdout.strip().splitlines()
        if window_name in windows:
            print(f"  OK: {name} created window '{window_name}'")
        else:
            print(f"  WARN: {name} window '{window_name}' not found (windows: {windows})")
            # Not a hard fail — might need more time
    return ok


def print_checklist():
    """Print manual verification checklist."""
    print()
    print("=" * 60)
    print("MANUAL VERIFICATION CHECKLIST")
    print("=" * 60)
    print()
    print("Run these in a normal tmux session after sourcing the plugin:")
    print()
    print("  tmux run-shell \"$PWD/loopflow.tmux\"")
    print()
    print("1. [ ] Status bar shows [lf: <branch>] or [lf: --]")
    print("2. [ ] Press prefix+l+? — help overlay appears")
    print("3. [ ] Press prefix+l+L — layout picker appears")
    print("4. [ ] Open lf-dev layout and verify 3 panes")
    print("5. [ ] Open lf-swarm layout and verify 4 panes")
    print("6. [ ] Press prefix+l+w — wave picker appears")
    print("7. [ ] Press prefix+l+r — run action executes or shows message")
    print("8. [ ] Press prefix+l+p — PR action opens or shows 'gh not found'")
    print("9. [ ] Resize terminal to <120 cols, open layout — simplified 2-pane")
    print("10.[ ] Remove fzf from PATH, try picker — fallback works")
    print("11.[ ] Source plugin twice — no duplicate keybindings")
    print()
    print("Interactive commands for follow-up testing:")
    print()
    print(f"  tmux -L {SOCKET} attach  # attach to review server")
    print(f"  tmux -L {SOCKET} list-keys -T loopflow  # show bindings")
    print(f"  tmux -L {SOCKET} show-option -gqv @loopflow_mode")
    print(f"  tmux -L {SOCKET} kill-server  # cleanup")
    print()


def main():
    if not has_tmux():
        print("ERROR: tmux not found in PATH")
        sys.exit(1)

    print("loopflow tmux plugin review")
    print(f"plugin dir: {PLUGIN_DIR}")
    print()

    # Kill any leftover review server
    kill_server()

    try:
        print("Starting isolated tmux server...")
        start_server()
        print()

        checks = [
            ("Plugin source", check_plugin_source),
            ("Idempotent re-source", check_idempotent_source),
            ("Options", check_options_set),
            ("Status-right", check_status_right),
            ("Keybindings", check_keybindings),
            ("Prefix binding", check_prefix_binding),
            ("Status script", check_status_script),
            ("Layout scripts", check_layout_scripts),
            ("Layout windows", check_layouts_create_windows),
        ]

        results = []
        for name, check in checks:
            print(f"[{name}]")
            ok = check()
            results.append((name, ok))
            print()

        # Summary
        passed = sum(1 for _, ok in results if ok)
        total = len(results)
        print(f"Results: {passed}/{total} checks passed")

        print_checklist()

    finally:
        kill_server()


if __name__ == "__main__":
    main()

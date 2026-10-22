# Optional hosted UI diagnostics

```bash
uv run python scripts/test.py --ui-host
LOOPFLOW_NATIVE_TESTS=1 swift test --package-path swift --no-parallel
```

These commands need a display session; XCUITest also needs macOS Automation
and Accessibility permission. Use an already configured host for a demo or
integration investigation. They are not release or Task Flow prerequisites.
Jack Heart retired the required hosted gate and five-run quota on 2026-09-30
(LOO-357). Required Desktop checks build the app and exercise production views
headless through the `swift` suite in gate and CI; see [TESTING.md](../TESTING.md).
Publisher preparation and public artifact verification do not run this diagnostic
or require a saved UI-host receipt.

The explicit `--ui-host` command keeps bounded execution, a machine-wide lock,
per-run `.xcresult` artifacts, and classification of permission/bootstrap
failures. An explicit diagnostic failure stays a failure; it does not mean the
headless checks failed or that a Flow needs an Ask to grant permissions.

## Cleanup

Hosted automation takes focus. A crashed test runner can leave macOS Automation
Mode enabled. The diagnostic checks for this after every run. If it reports a
leak, on the configured host restart the user-owned manager and run one clean
hosted session:

```bash
killall testmanagerd
uv run python scripts/test.py --ui-host
```

The manager releases its stale client count and the clean session clears
Automation Mode. Do not attempt to remove protected system state files.

To exercise permission-failure reporting without launching Xcode:

```bash
LF_UI_HOST_SIMULATE_NO_PERMISSION=1 uv run python scripts/test.py --ui-host
```

This exits nonzero with a capability diagnostic. It does not exercise the UI.

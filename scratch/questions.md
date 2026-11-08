- Command name `scripts/check_desktop_launch.py` (standalone script sharing the
  release check's launcher) chosen over a `release-loopflow.py` subcommand: the
  PR check builds no signed package. It owns the launcher; release imports it.
- Launching NSApplication on the hosted macOS runner is treated as headless
  under AGENTS.md's required-check rule (no dialog, permission prompt or
  person; the release check already does it unattended). Unconfirmed.

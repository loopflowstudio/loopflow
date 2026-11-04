# Decisions

## 2026-10-30 — Dev installs a signed Loopflow.app

**Context:** `lf install --dev` linked the CLI alone, so on a Mac the terminal ran green main while Desktop kept its released bundle. During the v0.15.1 release, a dev CLI migrated the store and the app's bundled lf could no longer open it.

**Decision:** Jack Heart asked that dev carry the app (LOO-688). `Packages (main)` builds, Developer ID signs and notarizes Loopflow.app for each green main commit; on a Mac `lf install --dev` verifies it and promotes the CLI and app together, linking `~/.local/bin/lf` into the app as preview and stable do. Dev runs no app updater; `lf install --dev` and the schedule move it. `install.py local` embeds Sparkle through the same bundling code as release builds.

**Implications:** LOO-723 removes the remaining CLI-only split: `LF_INSTALL_CLI_ONLY=1` skips first-time app setup, while an existing app stays paired with its CLI. CLI and scheduled installs complete through receipt-backed promotion; Session helpers keep running until their own retirement policy permits movement.

# Decisions

## 2026-10-30 — Dev installs a signed Loopflow.app

**Context:** `lf install --dev` linked the CLI alone, so on a Mac the terminal ran green main while Desktop kept its released bundle. During the v0.15.1 release, a dev CLI migrated the store and the app's bundled lf could no longer open it.

**Decision:** Jack Heart asked that dev carry the app (LOO-688). `Packages (main)` builds, Developer ID signs and notarizes Loopflow.app for each green main commit; on a Mac `lf install --dev` verifies it and promotes the CLI and app together, linking `~/.local/bin/lf` into the app as preview and stable do. Dev runs no app updater; `lf install --dev` and the schedule move it. `install.py local` embeds Sparkle through the same bundling code as release builds.

**Implications:** Only `LF_INSTALL_CLI_ONLY=1` still links the CLI alone on a Mac.

## 2026-11-04 — Regressions are GitHub issues

**Context:** v0.15.1 read `regressed_in` from the release Machine's own Tasks, so a mark made on another Machine never held `stable`. After a release PR merged, `lf release run` also failed on a merge commit it had not fetched (LOO-691).

**Decision:** Jack Heart chose the GitHub issue as the record: a `regressed-in:vX.Y.Z` label on an issue in the released repository holds `stable`, and Tasks carry no regression field. `--regressed-in` leaves `lf task create|edit`. `lf release issues`, `regress` and `promote` own regressions and moving `stable`, which stays GitHub's Latest marker. Jack also chose the installed `lf` for host releases, with no separate release home; the dedicated host follows `dev`, so release fixes reach it from green main.

**Implications:** When issues can't be read, the release publishes, `stable` holds, and `lf release check` and `lf release promote` fail. `tasks.regressed_in` and its plan-ref field stay, with nothing acting on them, so stored rows and released peers keep decoding until LOO-730 drops both.

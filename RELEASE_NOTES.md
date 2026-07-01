# v0.9.12

A maintenance release aimed inward, at the machinery that builds, updates, and ships loopflow. Local development and the native Mac host now share one update command instead of two drifting scripts. The DMG release job can no longer hang the pipeline silently. And release notes — including this one — are now written by interpreting the decision ledger against what actually shipped. Nothing here changes the product surface; it makes that surface easier to keep current and to release.

## One update path for local and native hosts

Local dev and the native Mac `lfd` host had grown two implementations of the same operation: pull the default branch, rebuild `lf`/`lfd`, install them into a local bin dir. Two copies meant two things to test and a standing risk that the Mac host would drift from the developer path. This cycle collapses them onto one. `scripts/install.py` now owns local installation end to end, and the native host updater calls it directly.

- **`install.py refresh` is the fast CLI-only update** — pulls the default branch (fast-forward only, and it refuses to pull a non-default branch), rebuilds `lf`/`lfd`, installs them atomically. `--no-pull` and `--install-dir` are there when you need them.
- **`install.py local --use` stays the full build-and-promote path** for `lf`, `lfd`, and `Loopflow.app`.
- **`deploy/native-lfd-host.sh` updates via `install.py refresh`**, so the Mac host and a developer's laptop run the same code.
- **`scripts/pull-local-bin.sh` is now a thin wrapper** that delegates to `install.py refresh` — existing callers keep working; new docs point at `install.py`.

## A release pipeline that fails fast and explains itself

Two changes harden how a release actually gets out the door. The DMG job could hang indefinitely on a stuck subprocess and spin until the workflow-level limit with no signal why; now every external step is bounded and logs are unbuffered, so a stuck step fails fast with a clear message. Separately, the release-note step was rebuilt around the split this document is written to: `DECISIONS.md` carries intent, merged PRs and diffs carry shipped behavior, and the note interprets one against the other.

- **DMG build hangs are bounded** — `build-dmg` gets a 45-minute cap plus tighter per-step limits (35m build/sign/notarize, 30m `notarytool --wait`, 20m cargo/swift, 10m dmg creation, 5m codesign/staple, 5m R2 upload). The release script runs with `python3 -u` and flushed logging, so CI progress is visible as it happens instead of after the fact.
- **Release notes fuse intent and behavior** — the `release-notes` step treats decisions as the *why* and commits as the *what shipped*, producing an outcome-oriented story (opening → thematic sections → operational notes → small changes). The raw ledger stays archived under `release/v<version>/DECISIONS.md`.
- **`lf op release notes` and `lf op release run` share one step** — standalone notes, weekly releases, and repo consumers like Cadenza now use the same prompt contract instead of each embedding its own note writer.

## Operational notes

- Deploy scripts and docs that referenced `pull-local-bin.sh` still work through the wrapper, but should migrate to `scripts/install.py refresh`.
- No config, DTO, or schema changes this cycle; upgrading is a rebuild.

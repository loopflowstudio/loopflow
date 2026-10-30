# LOO-688: `lf install --dev` installs a signed Loopflow.app

Stacked on LOO-669 (#1776): this branch rebases onto its head, since `--dev`, streams and
the app updater live there. Land #1776 first; the Task's stack placement is not recorded
in the store (`lf task run --stack-on` would also launch a run).

Built 2026-10-30:
- `Packages (main)` `desktop-app` job (macos-26): imports the Developer ID cert into a
  temp keychain, runs `scripts/release-loopflow.py` with `LF_RELEASE_BINARY` = the ARM64
  package's `lf` (promotion requires the app's helper to be the promoted candidate),
  validates the stapled DMG, uploads `Loopflow-dmg`.
- `published.rs` `main_build`: on a Mac (not `LF_INSTALL_CLI_ONLY`) downloads that
  artifact, mounts it, `codesign --verify --deep --strict` + `spctl --assess`, promotes
  CLI and app together; `DiskImage` detaches on drop.
- `bundle_version.read_source_revision` prefers checkout HEAD: in `workflow_run`,
  `GITHUB_SHA` is main's newest commit, not the packaged one.
- Actions secrets set: APPLE_CERTIFICATE_BASE64, APPLE_CERTIFICATE_PASSWORD, NOTARY_KEY,
  NOTARY_KEY_ID, NOTARY_ISSUER.

Remaining:
- First real CI run of `desktop-app` (keychain import, notarization on a hosted runner,
  the packaged-app launch check in `release-loopflow.py`). `gh workflow run
  main-packages.yml --ref <branch>` proves it before merge. Owner: gate/demo.
- Demo: `lf install --dev` on a Mac with Loopflow.app running → app quits, is replaced,
  reopens; `lf --version` and the app's `LoopflowSourceRevision` name the same commit.

Check: `cargo test -p loopflow --lib lf::commands::install::` 30 pass; clippy pass (after compress).

Concept review 2026-10-30 (after compress): nothing to change; no open product choice.
Jack's steer on infrastructure MEMORY.md (`lf install --dev` alone advances `~/.lf`) is
already in `abe4ee458e`.

Steers 2026-10-30 (after sync onto main, #1776 merged):
- MEMORY.md `lf install --dev` phrase: already done.
- "delete loopflow.db" refusal: gone on main since v0.15.0 (`incompatible()` says keep it, or run `lf install`).
- `install.py local` Sparkle: `scripts/app_bundle.embed_frameworks` shared with release packaging.
- Preview install output: the link was right (into the app); the "promoted" line printed the staged
  `~/.lf/bin` copy and the Desktop warning fired after replacing the app. Both fixed.

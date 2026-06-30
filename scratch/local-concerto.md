# Local build story + Loopflow rename

Make the Loopflow desktop app a first-class citizen alongside `lf` in both the
local-build story and the `release/` deployment story. Rename the user-facing
product from "Loopflow Concerto" to "Loopflow"; keep "Concerto" as the internal
dev nickname.

## Decisions (locked with Jack)

- **Single local build entry: `install.py`, repointed to a per-worktree `local-bin/`.**
  One command builds `lf`, `lfd`, and `Loopflow.app` into `<worktree>/local-bin/`.
  Each worktree gets its own isolated build — no more fighting over global
  `~/.local/bin` and `/Applications`.
- **Build + promote model.** Building always lands in `<worktree>/local-bin/`.
  `--use` *promotes* one worktree to active:
  - symlink `~/.local/bin/{lf,lfd}` → `<worktree>/local-bin/{lf,lfd}`
  - install `<worktree>/local-bin/Loopflow.app` → `/Applications/Loopflow.app`
- **Rename user-facing product → "Loopflow".** Display name, app bundle filename,
  DMG volname, and the DMG/R2 download keys all become `Loopflow`.
  - Keep bundle id `com.loopflow.concerto` (preserves TCC permissions, deep links).
  - Keep the Swift target / on-disk executable named `Concerto`.
  - Keep `concerto-dev.py` and `Concerto Dev.app` as-is (dev nickname).
- **App version is stamped from the release version**, not stored separately.
  Single source of truth stays `Cargo.toml`/`pyproject.toml`. The bundle's
  `CFBundleShortVersionString`/`CFBundleVersion` are stamped at build time from
  `RELEASE_TAG` (CI) or `Cargo.toml` (local). Today the app ships frozen at
  `1.0` regardless of release — that is the concrete "second-class" symptom.
- **DMG rename is cross-repo.** `Loopflow-latest.dmg` / `Loopflow-{version}.dmg`
  changes the public download URL; the loopflowstudio website link must update
  in the same window. Flagged, not done here.

## Work

1. **Rename (user-facing only):** `Info.plist` display+bundle name → Loopflow;
   `SetupView`, `ContentView` strings → Loopflow.
2. **`install.py`:** build into `local-bin/`; `--use` promote; version-stamp the
   bundle; `Loopflow.app`.
3. **`release-concerto.py` + `release.yml`:** `Loopflow.app` / `Loopflow.dmg`,
   R2 keys `Loopflow-*.dmg`, version-stamp from `RELEASE_TAG`.
4. **Docs:** `.gitignore` `local-bin/`; `release/README.md` + `release/SCHEDULE.md`
   name the app artifact + local updater as first-class; root `README.md` install
   section; `release/unreleased/DECISIONS.md` entry.

## Out of scope / flagged

- loopflowstudio website download-link update (separate repo).
- No reorg of `concerto-dev.py`; debug app stays `Concerto Dev.app`.

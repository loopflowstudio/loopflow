# fix/screenshot-signing

## What was implemented

Two changes to fix Concerto screenshot generation:

1. **Development team for code signing** — Added `DEVELOPMENT_TEAM: 2V3M244HF2` to `swift/project.yml` so Concerto builds are signed. Without this, the app bundle may lack entitlements needed for screenshot capture (window server access, etc.).

2. **Prefer installed Concerto** — Changed `find_concerto_executable()` in `scripts/generate_screenshots.py` to check `/Applications/Concerto.app` first, before scanning DerivedData. This avoids picking up stale or unsigned builds from DerivedData when a properly signed app is already installed.

Both changes together ensure `generate_screenshots.py` uses a signed, installed Concerto build. The refreshed screenshots in `docs/screenshots/` confirm the fix works (new `concerto-wave-failed.png`, updated `concerto-main.png`, `concerto-wave-running.png`, `concerto-wave-waiting.png`).

## Key choices

- **Hardcoded team ID in project.yml** — The team ID is already implicitly tied to this repo (it's the developer's Apple team). Putting it in project.yml means all developers with this team get automatic signing. Developers on other teams can override via Xcode's local signing settings.

- **Installed app takes priority over DerivedData** — The previous logic scanned DerivedData first, then fell back to `/Applications`. Reversing the order means a known-good installed build is always preferred. DerivedData builds are only used as a fallback (and get installed to `/Applications` when found).

## How it fits together

`generate_screenshots.py` → finds Concerto executable → launches with `--snapshot` → captures screenshots to `docs/screenshots/`. The signing fix ensures the executable has the entitlements needed for window capture. The lookup order fix ensures the signed build is found first.

## Risks and bottlenecks

- **Team ID portability** — Other contributors without team `2V3M244HF2` will see Xcode prompt for their own team. This is standard Xcode behavior and doesn't break builds (CI already passes `CODE_SIGNING_ALLOWED=NO`).

## What's not included

- No CI changes — CI already uses `CODE_SIGNING_ALLOWED=NO` for tests, so the team ID is ignored there.
- No changes to the snapshot capture logic itself.

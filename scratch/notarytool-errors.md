# LOO-894: say why notarization failed

## Problem

Nine `main-packages.yml` runs on 2026-11-13 failed with only "Notarization
upload did not return a receipt; inspect Apple history before retrying". The
cause was an expired Apple Developer agreement (HTTP 403), which no code change
fixes. Outcomes: the failing log names Apple's message
(`scripts/release-loopflow.py` `_notarytool_failed`), and a daily
`notarytool history` check (`.github/workflows/notarization-check.yml`) reports
breakage before package builds do. Fixes live in `.github/CI.md`, "Notarization".

Direction (Jack Heart, 2026-11-13), on the daily check that "would tell you the
morning it breaks instead of nine failed package builds later": "yeah soudns
good".

## Probe (2026-11-13, local, `loopflow`/`prd` Doppler key)

`xcrun notarytool history --key … --output-format json`:

| key id | exit | stdout | stderr |
|---|---|---|---|
| real | 1 | empty | `Error: HTTP status code: 403. A required agreement is missing or has expired. …` |
| `WRONGKEY123` | 1 | empty | `Error: HTTP status code: 401. Unauthenticated. Ensure that all authentication arguments are correct.` |

Errors arrive as plain text on stderr even with `--output-format json`. The
agreement was unsigned at planning time, so the check's first run will be red
until someone accepts it at developer.apple.com.

## Demo / remaining

1. `uv run pytest python/tests/test_release_loopflow.py -k notarytool`: the
   stubbed 403 text appears in the RuntimeError.
2. Next `main-packages.yml` desktop-app log (agreement still unsigned) shows
   `Error: HTTP status code: 403. …` above the traceback and
   `Notarization upload failed: Error: HTTP status code: 403. …`.
3. After merge, `gh workflow run notarization-check.yml` goes red with the 403
   and opens "Notarization is failing"; after the agreement is accepted, the
   next run goes green and closes it.

Exclusions: accepting the agreement, `notarytool log` on `Invalid`, retries,
alerting beyond the issue.

## Checks

- `uv run pytest python/tests/test_release_loopflow.py` — 17 passed (compress).
- `actionlint .github/workflows/notarization-check.yml` — clean (compress).
- Step shell with stub `xcrun`/`gh`: red+no issue → create, red+open → comment,
  green+open → close, green+none → nothing; exit codes match (implement).
- Workflow on GitHub — verify, after merge (demo 3).

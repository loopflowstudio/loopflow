# LOO-894: say why notarization failed

## Problem

Nine `main-packages.yml` runs on 2026-11-13 failed with only "Notarization
upload did not return a receipt; inspect Apple history before retrying". The
cause was an expired Apple Developer agreement (HTTP 403), which no code change
fixes. Outcomes: the failing log names Apple's message
(`scripts/release-loopflow.py` `_notarytool_failed`), and a daily
`notarytool history` check (`.github/workflows/notarization-check.yml`) reports
breakage before package builds do. Fixes live in `.github/CI.md`, "Notarization".
The brief's candidates for the issue step don't fit: `ci_incidents` is the
local store's PR-landing record and close-on-red is Dependabot's PR path, so the
workflow uses a plain `gh issue` step.

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
agreement was unsigned at planning time. By QA (2026-11-13, afternoon) it had
been accepted: `history` exits 0 and the three newest submissions (from 03:59Z
to 07:00Z) are `Accepted`. The check's first run will be green.

## Message shape

Apple's line says why; the advice says what is safe next. Every message carries
the advice (concept review, 2026-11-13):

```text
Notarization upload failed: Error: HTTP status code: 401. Unauthenticated. Ensure that all authentication arguments are correct; inspect Apple history before retrying
Notarization upload failed; inspect Apple history before retrying   # notarytool printed nothing
```

## Demo / remaining

1. `uv run pytest python/tests/test_release_loopflow.py -k notarytool`: the
   stubbed 403 text appears in the RuntimeError.
2. After merge, `gh workflow run notarization-check.yml` goes green
   ("notarytool history succeeded"). A red CI run now needs a broken secret;
   the wrong-key-id 401 path was run locally against Apple in QA.

Exclusions: accepting the agreement, `notarytool log` on `Invalid`, retries,
alerting beyond the issue.

## Checks

- `uv run pytest python/tests/test_release_loopflow.py` — 17 passed (compress, after dropping impossible `None` guards in `_notarytool_failed`).
- `actionlint .github/workflows/notarization-check.yml` — clean (compress).
- Step shell with stub `xcrun`/`gh`: red+no issue → create, red+open → comment,
  green+open → close, green+none → nothing; exit codes match (implement).
- QA against Apple with the real `prd` key (`scratch/qa.html`): release
  script's submit path with a wrong key id prints and raises the 401; a
  non-UDIF file surfaces notarytool's local validation error; the workflow step
  with real `xcrun` and a logging `gh` stub takes all four issue branches;
  Apple's trailing period no longer reads `correct.; inspect` (fixed in QA).
- Workflow on GitHub — verify, after merge (demo 2).

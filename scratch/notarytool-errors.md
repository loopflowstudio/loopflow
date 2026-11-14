# LOO-894: say why notarization failed

## Problem

`scripts/release-loopflow.py` `_notarize_dmg` discards notarytool's output on a
nonzero exit. Nine `main-packages.yml` runs on 2026-11-13 failed with only
"Notarization upload did not return a receipt; inspect Apple history before
retrying". The real cause was an expired Apple Developer agreement (HTTP 403),
which no code change fixes. Two outcomes: the failing log names Apple's
message, and a daily check reports breakage before package builds do.

Direction (Jack Heart, 2026-11-13), on the daily `notarytool history` check
that "would tell you the morning it breaks instead of nine failed package
builds later": "yeah soudns good".

## Demo

1. `uv run pytest python/tests/test_release_loopflow.py -k notarytool` passes;
   the stubbed 403 text appears in the RuntimeError.
2. The next `main-packages.yml` desktop-app log (while the agreement is still
   unsigned) shows `Error: HTTP status code: 403. A required agreement is
   missing or has expired…` above the traceback, and the exception reads
   `Notarization upload failed: Error: HTTP status code: 403. …`.
3. After merge, `gh workflow run notarization-check.yml` goes red with the 403
   and an open issue "Notarization is failing" quotes it. Once the agreement is
   accepted, the next run goes green and closes that issue.

## Probe (2026-11-13, local, `loopflow`/`prd` Doppler key)

`xcrun notarytool history --key … --output-format json`:

| key id | exit | stdout | stderr |
|---|---|---|---|
| real | 1 | empty | `Error: HTTP status code: 403. A required agreement is missing or has expired. …` |
| `WRONGKEY123` | 1 | empty | `Error: HTTP status code: 401. Unauthenticated. Ensure that all authentication arguments are correct.` |

So errors arrive as plain text on stderr even with `--output-format json`, and
the agreement was still unsigned at planning time: the daily check's first run
will be red until someone accepts it at developer.apple.com.

## Approach

### Script (`scripts/release-loopflow.py`)

- One private helper, `_notarytool_failed(result, what, advice) -> RuntimeError`:
  prints captured stdout then stderr (each only if non-blank), takes the first
  non-empty line of stderr, else stdout, and returns
  `RuntimeError(f"{what}: {line}")`; with no output at all, falls back to
  `f"{what}; {advice}"`.
- `submit` nonzero → `raise _notarytool_failed(result, "Notarization upload failed",
  "inspect Apple history before retrying")`. The receipt keeps `submission_id:
  None`, so the "outcome unknown" path on retry is unchanged.
- `wait` nonzero → same helper with `f"Notarization {id} wait failed"` and the
  existing "retry resumes the retained submission" advice.
- No retries. Output is notarytool's own; the key is a temp file path, never
  its contents.

### Daily check (`.github/workflows/notarization-check.yml`)

- `on: schedule: cron '17 13 * * *'` (morning US) and `workflow_dispatch`.
  `runs-on: macos-26` like `main-packages.yml`; `permissions: issues: write`.
- One step: write `NOTARY_KEY` to `$RUNNER_TEMP/notary.p8`, run
  `xcrun notarytool history … --output-format json > /dev/null 2> err`, cat
  `err` into the log.
  - On failure: find the open issue with `gh issue list --state open --search
    'in:title "Notarization is failing"'`; comment Apple's message on it, or
    `gh issue create` with that title, the message and a link to the run.
    Exit 1.
  - On success: close that issue (if open) with a comment linking the run.
- `close-on-red` in `dependabot-auto.yml` is PR-shaped (match by head SHA,
  comment, close); nothing named `ci_incidents` exists. Plain `gh issue` steps
  inline, no shared action.
- The repository is public and Issues are enabled; Apple's message carries no
  secret.

### Docs

- `.github/CI.md` gains a "Notarization" subsection under Main packages: where
  the error prints, the daily check and its issue, and that a 403 "required
  agreement" means the Account Holder accepts the agreement at
  developer.apple.com (no code change, no retry). 401 means a wrong
  `NOTARY_KEY_ID`/`NOTARY_ISSUER`/`NOTARY_KEY`.
- The script header points at that section in one line.

## Exclusions

- Accepting the agreement itself (Apple account action, not code).
- `notarytool log` on an `Invalid` status; the existing status message stays.
- Retries, alerting beyond the GitHub issue.

## Done when

- A failed submit or wait prints notarytool's output and raises with its first
  line; empty output keeps the "inspect Apple history" advice.
- `notarization-check.yml` runs daily and on dispatch, fails with Apple's text,
  and opens/updates/closes one "Notarization is failing" issue.
- `.github/CI.md` names the workflow and the 403 fix.

## Checks

- `uv run pytest python/tests/test_release_loopflow.py` — 17 passed (implement),
  incl. `test_notarytool_failure_names_apples_message` (submit 403, wait 401,
  empty output keeps the advice).
- `actionlint .github/workflows/notarization-check.yml` — clean (implement).
- Step shell run locally with stub `xcrun`/`gh`: red+no issue → create, red+open
  → comment, green+open → close, green+none → nothing; exit codes match (implement).
- Workflow on GitHub — verify: after merge, `gh workflow run notarization-check.yml`
  should go red with the 403 and open the issue while the agreement is unsigned.

## Docs written

- `.github/CI.md` — "Notarization" under Main packages (log shape, daily check,
  issue, 403/401 table, empty-output advice).
- `scripts/release-loopflow.py` — header line pointing at that section.

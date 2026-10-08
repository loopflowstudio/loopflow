PR #1496 failed at `4ea6d0a10e079949ae9b63caa273c784b24ad89f` in run 37717479135: fallback-font touch targets and a Swift fixed-delay assertion. Sync was already current; repair preserves homepage copy and observes Swift publication directly.

Checks: `cd website && uv run python dev.py test` — 76 passed; original CSS reproduced both 43px failures; `scripts/test_desktop.sh -Xswiftc -gnone --filter ProgramStatusTests` — 11 passed; full Swift CI remains pending on publication.

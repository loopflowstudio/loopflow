# Design Review: publish-failure

Branch: `jack-heart.publish-failure.20260130_1941`

## What was implemented

Two fixes to the publish script that handle race conditions and path issues:

1. **PyPI race condition handling** — When publishing to PyPI, a "File already exists" error is now treated as success rather than failure. This handles the race where `_is_on_pypi()` returns false but another process publishes before we do.

2. **DMG build invocation fix** — Changed `build_dmg()` from invoking `./dev release` to `sys.executable scripts/dev.py release`. The old `./dev` shell script no longer exists; it was replaced by `scripts/dev.py`.

## Key choices

**PyPI race handling**: The fix checks for "File already exists" in the error output and treats it as an idempotent success. This is the correct approach because:
- The goal is to ensure the version is on PyPI, not to be the one who published it
- Multiple parallel releases or retries should converge to success
- Alternative (locking) would add complexity for a rare edge case

**DMG build path**: Using `sys.executable` ensures the correct Python interpreter runs the script, regardless of how the publish script itself was invoked.

## How it fits together

The publish script already had idempotency for tagging (check if tag exists before creating). These changes extend that pattern to PyPI publishing and fix a broken path reference.

## Risks and bottlenecks

- The "File already exists" string match is brittle — if PyPI changes their error message, we'd fail again. However, this is a defensive fallback; the primary check (`_is_on_pypi()`) should prevent most cases.
- No test coverage for the race condition path (mocking PyPI responses would be complex).

## What's not included

- No changes to the release PR creation flow
- No changes to DMG upload logic
- The root cause of why `./dev` was replaced isn't addressed here — this just fixes the symptom

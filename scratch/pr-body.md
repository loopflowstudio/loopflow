## Try it!
- `target/debug/lf --list | sed -n '/gstack/,+31p'`
  - See the imported `gstack:*` workstyle steps in the normal step listing.
- `target/debug/lf-prompt --repo . --step gstack:office-hours --surface headless --lfdocs false --diff false --diff-files false | rg 'gstack:(ceo-review|eng-review|design-review)'`
  - Verify the imported prompt now references loopflow-native follow-on steps instead of old `/plan-*` commands.
- `uv run pytest python/tests/test_workstyle_convert.py -v`
  - Covers converter output, direction extraction, and the new reference-rewrite behavior.

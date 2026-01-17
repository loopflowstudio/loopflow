# Open Questions

## Chrome Integration (`--chrome` flag)

**Question**: Should loopflow pass `--chrome` to Claude Code by default?

**Answer**: No, not by default.

The `--chrome` flag enables Claude Code to connect to and control Chrome for browser-based tasks (testing web apps, reading console logs, automating browser workflows). According to Claude Code docs:
- Requires Chrome extension (v1.0.36+)
- Requires paid Claude plan
- Increases context usage
- Only works with Google Chrome (not Brave, Arc, etc.)

Most loopflow tasks (design, implement, review, polish) work with files and git—they don't need browser access.

**Possible future enhancement**: Add a `chrome: true` config option or `--chrome` CLI flag for tasks that need it, like:
- UX verification tasks that check rendered UI
- E2E test tasks that interact with web apps
- Screenshot-based debugging workflows

This would be opt-in per-task or per-repo, not a global default.

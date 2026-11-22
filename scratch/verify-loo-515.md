# LOO-515 verify (2026-11-21, headless)

Outcome: verified with one follow-up (ab6882). Advance.

- PR #1949 merged 2026-11-22T07:51Z as 15cf97ce82; its tree equals the verified head 3e000deaff (`git diff --stat` empty). All required CI passed on that head, including macos swift-test, loopflow-ui-test and both macOS process tests.
- lf built from that tree, isolated LF_HOME: `lf config catalog` lists `diff_hidden` (AppendReset) and `diff_groups` (Replace, with rest); `lf help tour` resolves the builtin; `details` is a Desktop registry action (desktop.rs:2429); TaskPullRequestView.swift is gone (no second PR renderer).
- Task state: PR phase merged and Task still open. Merge didn't complete it.
- Gap vs the Nov 19 steer: groups replace instead of stacking tour > personal > repository > built-in, and they're two flat keys instead of one `diff` section. Chosen while building and flagged as an open demo question in the PR; filed as follow-up ab6882 with the steer's words.
- Not run here: any look at it on screen (Brief/Diff/Files at 360–640 pt, review loop, keyboard). Needs a person at Desktop. The PR's "Unseen slices" go to c9fbcb (Diff v2) and the demo.

Terminal feedback could obscure text or flatten distinct colors, while navigation took space away from the conversation. This change makes Copy and Find feedback brief, opens navigation on demand, and keeps files and published GitHub PRs beside the retained Session.

## What changes

- Copy confirms successful clipboard writes in the pane header without decorating cells. Find briefly emphasizes the current match and respects Reduce Motion. Exact Command-C/V/F ownership preserves provider shortcuts with extra modifiers.
- Ghostty composes shell backgrounds before contrast correction. Low-contrast text retains more color; `minimum-contrast = 1` preserves original colors. The immutable lf7 artifact is checksum-pinned.
- Work navigation starts closed, opens with Command-K, and can be pinned. Files uses the same dropdown/pin pattern with Command-P, showing scratch and changes against the recorded parent commit.
- New Task workspaces open Session and Files at 61.8/38.2. Dividers resize with constrained golden-ratio snapping. Companion menus replace content in place while retaining file drafts and hidden shells.
- The compact PR viewer reads GitHub status, checks, requested reviewers, description, change totals and expandable patches. Missing access and incomplete patches stay explicit; removing the PR link clears the retained page.
- Pane controls support alternating terminal splits, Claude/Codex/OpenCode switching, Session reset and compact Task lifecycle actions.

Layouts persist within the window, not across app relaunch. Contrast correction cannot guarantee exact hue, separation of nearly identical colors, or contrast through transparent backgrounds and custom postprocessing.

## Checks

Architecture, platform-boundary, Ruff and whitespace checks passed. Earlier focused Swift and renderer checks passed. The latest completed Swift run exercised 568 tests and exposed only three navigation-inspection issues across two tests; this head repairs their control lookup using the same concrete-button approach as the passing parent tests. Earlier CI repairs cover Ghostty-only assertions in the Xcode fallback and the current Available fixture vocabulary. The Xcode UI build and the remaining completed CI jobs passed on the preceding head; CI must validate this final head.

Local test/build preflight could not meet the repository's disk-space floor, so current executable verification is owned by CI. Jack Heart approved the revised demo. The [accepted design and verification record](https://github.com/loopflowstudio/loopflow/blob/aabba6342d/scratch/terminal-polish.md) retains the renderer evidence and limits. Paired native performance and all-provider input scenarios remain unmeasured; no performance improvement is claimed.

## Try it

Suggested walkthrough: open a Task, use Command-K to select Work, then Command-P in Files to select a changed file. Temporary choosers should close after selection. Switch the companion to PR and expand Files changed to inspect published patches.

Copy a multiline terminal selection and navigate Find matches. Return after switching companions to check retained file drafts and shell contents. These individual scenarios have not all been verified natively.

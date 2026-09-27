# LOO-309 assumptions and unresolved evidence

2026-09-26. Working design: [The docs read in the new voice](the-docs-read-in-the.md).

- **Confirmed scope:** Jack corrected the review on 2026-09-26: “Unbounded size
  per PR.” Implement the full Task; there is no Conducting-only delivery or
  page-count limit. This confirms scope, not new copy. See
  [review feedback](docs-voice-feedback.md).
- **Interpretation:** Glossary acceptance covers product concepts and borrowed
  engineering terminology beyond CS 101, including explanatory table text and
  comments. Ordinary English and individual CLI tokens do not each need entries.
  First-use explanations still describe what unfamiliar commands and fields mean.
- **Constraint:** No rendering environment is supplied. Source and help checks
  can establish behavior contracts; they cannot prove a Mac app walkthrough.
  Keep established app orientation, write no new click-by-click path, and leave
  the first-task/terminal walkthrough with LOO-306.
- **Unperformed proof:** No non-engineer read-aloud or app interaction occurred.
  Keep those limitations explicit in delivery evidence; do not manufacture reader
  feedback or substitute tests for comprehension.
- **Stale context:** PR #1297 is already the base commit here. Growth memory's
  old open-PR question is resolved for this checkout. Its flow-model observation
  also predates code now present, but this inspection does not prove LOO-317.
- **No coordination change needed:** The Wave's pause-state discrepancy does not
  prevent this assigned local docs design. Do not alter its schedule or planning
  state as a prerequisite.

- **Resolved accuracy gap:** Managed Task delivery uses vendor bypass settings
  after required-access checks; it does not enforce the worktree-only sandbox
  previously claimed by Configuration. Source evidence is in the design ledger.
  The docs correction stays within the authorized accuracy scope.
- **Updated test availability:** The supplied run has no displayed UI, but its
  installed Playwright Chromium supports unattended browser tests. The first
  focused run reached all tests (19 passed, one stale wording assertion).
  This is website rendering evidence only, not a Mac app walkthrough.

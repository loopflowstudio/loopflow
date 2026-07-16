## Try it!

1. Select the `product` Wave and open **Context Lab** from its header.
2. Open **Sources** and select `headless surface` or `wave_pursue`.
3. Confirm main's current file fills the primary pane beside impression and
   revision evidence.
4. Choose a Refinement Project if prompted.
5. Click **Refine in task-worker** only when ready to create a real Task.

## Intent

Context Lab makes the text shaping a Wave's agent sessions inspectable and
editable. It separates initial-prompt load, lifetime provider input, and peak
request pressure, then pairs those measurements with a Sources worklist ranked
by agent-session impressions.

## Key decisions

- Context Lab opens from a selected Wave, with repo and Wave fixed for the
  window's lifetime. It is progressive disclosure from the primary Wave UI,
  not a global destination.
- Selecting a source opens main's current file as the primary object;
  historical revisions and representative trace addresses remain evidence.
- Rust owns session reconciliation, source identity, impressions, provider
  normalization, and coverage. Swift renders one atomic snapshot and owns
  interaction state.
- **Refine in task-worker** creates and starts a normal Project-owned Task and
  opens the existing Task workspace on its Agent view. Single-Project Waves
  route automatically; multi-Project Waves remember an explicit Refinement
  Project.
- The handoff refreshes the Wave plan and Context Lab snapshot, validates the
  source receipt, then rechecks main's source immediately before Task creation.

## Assumptions

- An impression is one distinct agent session whose captured initial prompt
  contains the source at least once.
- Missing provider or source-level capture stays unavailable rather than
  becoming zero.
- Cost remains outside Context Lab; context-window pressure is the comparable
  capacity signal.

## Not included

- No embedded Markdown editor, alternate agent host, synthetic quality score,
  or hidden Task lifecycle.
- Automated review stops before the Task-creation click. The resulting live
  agent session, source diff, and backlink are the human-review checkpoint.

## Verification

- Full repository gate passed after rebasing onto current main.
- Python: 59 passed.
- Rust: formatting and clippy passed; 1,409 tests passed, 2 skipped.
- Website: 59 passed, 3 skipped.
- Swift: 133 tests across 23 suites passed; multiplatform boundaries passed.
- E2E smoke passed; Xcode app/UI-test `build-for-testing` passed.
- Migration order and checksums passed through `0.11.012`;
  `git diff --check` passed.

# Settings demo — LOO-714 · 2026-11-03

Jack Heart requested demo, then asked for current status. No acceptance of the
implemented behavior has been given. Prior approval was of the design.

## Current state

[PR #1805](https://github.com/loopflowstudio/loopflow/pull/1805) is open at
4b424ecafa415f76c5901d112c2215ab6ea33f97. Pursue completed publication; no live
implementation Graph remains. Auto-merge is off and Auto-Ship is stopped. The
Graph's final completion attempt failed because the PR is unmerged; the Task
correctly remains open, with recovery still a separate second PR.

GitHub CI has eight failing tests in three Rust jobs, not a clean gate:

- preferred_name_survives_fresh_launches_and_corrections: repo user setting rejected.
- connect_overrides_personal_git_without_rebinding_the_repository: missing resolved
  planning.team after personal planning preference wins.
- global_agent_help_and_no_agent_diagnostic: expects old inherited-agent help text.
- graph_help_validates_expansion_and_review_boundaries_without_effects: duplicate
  authored review ID rejected.
- Four load_config_or_default tests expect no default agent but observe one.

Evidence: /tmp/loo-714-ci-failures.log; GitHub run 41629160735. These require
classification and repair; do not assume every failure is a stale assertion.
No code repair or Graph restart was performed as part of this demo.

## Observed through the CLI

Used the existing branch target/debug/lf with a disposable repository and LF_HOME;
removed inherited LF_* and left the live database/config untouched. Eight commands
exited successfully. Evidence identifies the binary by SHA256; this is a local
candidate demonstration, not proof of installation or exact final-head build.

- A personal skills.implement.agent=codex selects Codex.
- A personal release/write-code override selects Claude with chrome=false; another
  implement occurrence retains Codex with repository chrome=true.
- Composing release into outer preserves both targets and values.
- DB set for release/write-code selects Codex; unset restores personal-file Claude.
- Explicit CLI Codex beats environment Claude in config inspection.

Demo defect: that final inspection also warns “This command does not start an
agent; --agent has no effect.” Its output changes to Codex, so the warning is
misleading for config show. Proposed repair: acknowledge inspection's hypothetical
invocation context, suppressing or correcting this warning for effective-settings
reads. This is an observed issue, not an accepted new product requirement.

No real provider was launched. Session-to-Task separation and forced Graph launch
remain supported by the worker's recorded stand-in tests, not newly demonstrated
with authenticated providers here. Non-agent settings were exercised with chrome.

## Review record and next action

Review page: scratch/demo.html. Raw commands/results: scratch/settings-demo-evidence.json.
Current design: scratch/settings-scope.md. User-facing syntax: docs/config.md.

Jack has not yet supplied demo feedback; page notes are local drafts until shared
in this conversation. No new design changes have been agreed. Recommended next:
review the scoped-preference experience here, then repair the eight CI failures and
misleading inspection warning, run focused regressions, publish the corrected head,
and verify CI before any merge decision. Recovery remains for PR2.

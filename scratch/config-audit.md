# LOO-714 configuration audit — November 2–3, 2026

Nonsecret observations for Jack Heart, not new policy. Complete original audit:
`04bdec77e3:scratch/config-audit.md`. settings-scope.md owns the accepted inventory.

- Personal `harness: codex` was unrecognized and corrected to `agent: codex`, with
  other values preserved. Jack selected Codex temporarily after Claude exhaustion;
  no permanent/per-skill policy was invented. No personal repo agent override or
  DB agent/skill_agents rows were found. Legacy tasks.agent=claude is migration
  input, not future authority; live Task/history rows were not patched.
- Native defaults were Codex gpt-6-astra/high and Claude opus. They stayed unchanged;
  managed homes may differ. Inspected Loopflow/manabot repo/Wave/Graph YAML had no
  agent/model choice; Jack's removal of manabot's agent was preserved.
- November 3: Jack moved explore/kickoff/prompt/realign's Claude choices and
  xor-route's Claude Sonnet choice from builtins into repository config.
- Ordered routes: Claude default/repo loopflow, primary; Codex default
  jack-42d1021d3f2d, engineering, manabot-eng, jackstah-1066ea9c99d1; Loopflow repo
  Codex manabot-eng. These are account IDs, not credentials; preserve order.

Branch inspection with inherited LF variables removed selected personal-general
Codex for implement/explore/kickoff/prompt/realign/xor-route in both repositories,
without adding a model suffix. No comparative cost/quality run supports these
choices. Limit evidence remains in weekly-limit-2026-11-02.md.

Prelaunch overlaps: LOO-709/#1804 (timeout), LOO-500/#1803 (Graph/Task/DTO),
LOO-691/#1780 (release/config/auth), LOO-312/#1656 (setup/launch/auth). These are dated
observations, not current PR status or recovery decisions. Preserve their changes.
Separate loopflow-critique artifacts remain another contribution, outside this PR.

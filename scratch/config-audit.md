# LOO-714 configuration audit — 2026-11-02

Nonsecret preference audit for Jack Heart. Selection implementation must preserve
native defaults/account order; values below are observations, not new repo policy.

- Personal ~/.lf/config.yaml had harness: codex, no agent/skill_agents/skills/graphs
  selection. Current code recognizes agent, not harness. Corrected to agent: codex
  with all other parsed values preserved. Jack chose Codex temporarily because
  Claude usage is exhausted. No new per-skill preference was invented.
- Personal repos entry for /Users/jack/src/loopflow had no agent/model override.
- scope_settings had no agent or skill_agents.* rows at any scope.
- LOO-714 still has legacy tasks.agent=claude. The first PR removes this implicit
  future authority; do not patch live Task/history rows directly. This run explicitly
  selects Codex. Other Tasks' saved values are migration inputs, not user policy.
- Native Codex default: gpt-6-astra, reasoning effort high. Native Claude default:
  opus. Neither changed; managed account homes may have their own native defaults.
- No agent/model settings were found in inspected Loopflow/manabot repo/Wave/graph
  YAML. Jack's uncommitted removal of manabot's agent: codex is preserved.
- Builtin defaults: explore, kickoff, prompt, realign use claude; xor-route uses
  claude:sonnet. Implement has no builtin agent preference. No claim these choices
  are cost-optimal; use execution evidence before changing model assignments.
- Account route IDs (not credentials): default Claude loopflow, primary; default
  Codex jack-42d1021d3f2d, engineering, manabot-eng, jackstah-1066ea9c99d1. Loopflow
  repo Claude loopflow, primary; Codex manabot-eng. Preserve their order.

The branch CLI's read-only inspection, with inherited LF_* removed, resolves
implement, explore, kickoff, prompt, realign and xor-route to `codex` in both
Loopflow and manabot. Each winning candidate is the general personal
`~/.lf/config.yaml` preference. No model suffix is added, preserving the native
Codex default noted above. These are current selections, not evidence that Codex
is the cheapest or best model for each skill. No comparative cost/quality run was
performed; recorded account-limit evidence remains in weekly-limit-2026-11-02.md.
The complete setting-family inventory is in settings-scope.md.

## Overlapping open work

Observed before launch. Preserve upstream changes during refresh; these PRs do
not decide LOO-714's setting targets or failure recovery contract.

- [Idle unviewed agents · LOO-709 / #1804](https://github.com/loopflowstudio/loopflow/pull/1804)
  changes config/session timeout and docs. Do not restore an old timeout default.
- [Wave lifecycle · LOO-500 / #1803](https://github.com/loopflowstudio/loopflow/pull/1803)
  changes graph, Task, CLI and DTO surfaces.
- [Release regressions · LOO-691 / #1780](https://github.com/loopflowstudio/loopflow/pull/1780)
  changes config, Task/CLI and provider auth.
- [Mac setup · LOO-312 / #1656](https://github.com/loopflowstudio/loopflow/pull/1656)
  changes config, agent launch and provider auth.

Unrelated loopflow-critique.* artifacts belong to another contribution; preserve
but do not include them in the settings PR.

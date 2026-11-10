# Recovery choices

- 2026-11-10: Migrate manabot.repo's rejected `pm.provider` / `pm.linear_team`
  config to supported `planning.provider` / `linear.team`, preserving the team.
  This reversible local config change is needed to resume the eligible Session;
  the original is saved under `database-lock-evidence/manabot-config-before.yaml`.

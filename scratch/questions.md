# Open questions (LOO-1117)

Assumptions taken with nobody in the conversation; each is reversible.

- `DEPENDABOT_MERGE_TOKEN` moves into its own `dependabot` environment, beyond
  the five secrets the brief names. Undo: leave it at repository level and
  exempt it in the lint.
- `workflow_dispatch` on `main-packages.yml` is dropped rather than guarded on
  `refs/heads/main`.
- Environments `dev` (holds `LFD_GITHUB_WEBHOOK_SECRET`, 2026-02-13) and `pypi`
  (empty) are listed for deletion; no workflow names them. Is either still
  wanted?
- `DOPPLER_TOKEN_PRD` was already absent from repository secrets on 2026-11-26.
  Was the Doppler token behind it revoked? `loopflow`/`prd` lists no service
  tokens now.
- Rotate the Developer ID certificate and notary key? The one branch dispatch
  on record (run 41111152406, 2026-10-30) is the LOO-688 feature branch.
- Should the cron host run `scripts/check_actions_secrets.py` daily?
- GitHub's docs do not say outright that a Dependabot `pull_request_target`
  run reads environment secrets as it reads repository ones. Watch the first
  Dependabot PR after the merge.

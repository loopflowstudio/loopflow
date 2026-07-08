# Freshness

One command (and one cron) keeps every host current.

## KRs

- One command refreshes local `lf`/`lfd`/Loopflow.app and the maintained
  host; freshness failures surface as tasks, not silent drift (the
  sync-skills --global failure class — caught by log audit, should have been
  a task).
- The self-hosted `lfd` host stays up; main stays green.

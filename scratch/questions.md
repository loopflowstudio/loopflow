# Questions

- 2026-10-27 kickoff: Jack approved shipping PR #1711 from demo, but main's #1708 has since deleted the cron accounting that PR patches (conflict: release.rs, modify/delete on scheduled_release_tests.rs). Assumption: the approval covers fixing the cron outcome, not that specific diff. The plan drops the old diff on rebase and ships the launchd→crontab cutover in `scripts/bootstrap-cron-host.sh` instead. Reversible: the old diff is in git (1889cd12c1).

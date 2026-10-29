# LOO-669 open questions

- **Website follows preview, download follows stable (slice 2, 2026-10-29).** The
  publisher still deploys the website for every release, so its docs and
  `/healthz` describe `preview`, while its install command and "Download for
  Mac" serve `stable`. Assumed acceptable for now (docs describe a superset);
  revisit if a stable-only flag confuses readers.
- **First stable move.** Latest today is the newest release (v0.15.x). Forward-only
  means stable stays there until a newer release is 7 days old; no backfill.
- **Stable advance gates on `completion: github-release`**, not on having a
  publisher, so publisher-only targets in other repos keep no Latest marker
  after `--latest=false`. Loopflow's target sets `github-release`.
- **A done regression covers releases published before it closed (slice 3, 2026-10-29).**
  The fix release is the first release published after the regression Task
  completed; releases from `regressed_in` up to it stay ineligible. A release cut
  before the fix merged but published after the Task closed would count as fixed;
  assumed rare (a release run takes minutes). A done Task with an unreadable
  completion time keeps covering, so the failure is a stalled `stable` that the
  release log names (`LOO-1 keeps stable off v0.16.0, v0.16.1`), never a shipped bug.
  Abandoned or canceled regression Tasks cover nothing.
- **`regressed_in` reaches the release host only through local planning exchange.**
  It has no Linear field, so the cron Machine sees a mark set elsewhere only
  through peer planning (git). A released lf reading a peer planning document
  that carries `task_regressed_in` fails to parse it. Until the Linear label
  mapping (Open, design) lands, mark regressions on the Machine that runs the
  release, or after the release host's lf includes this field.
- **`--regressed-in` sets or replaces the mark; nothing clears it.** Complete or
  abandon the Task instead. Add `--not-regressed` if a mistaken mark needs undoing.
- **`lf release status` shows preview and stable, not dev (concept review, 2026-10-29).**
  dev is the newest green main build, and `lf install` (slice 4) owns it. Status
  stays about releases. Add a `Dev:` line if the release view feels incomplete without it.

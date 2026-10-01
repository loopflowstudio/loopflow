# Open questions and assumptions (LOO-341)

- **The cut list was not reviewed.** LOO-341 says "done when the agreed cuts
  ship" and has no comments. The cuts were chosen from the audit: delete every
  variable with no production reader, delete `LF_DB_PATH` as redundant with
  `LF_HOME`, and make one context list drive both session-shell and tmux
  scrubbing. Attribution of agreement is unresolved.
- **`LF_BIN` kept.** Under the main Home lf ignores it; under an explicit
  `LF_HOME` it names the experiment's binary and most fixtures depend on it.
  Deleting it means the experiment's binary becomes `current_exe`, which breaks
  in-crate tests whose executable is the test harness. Not attempted.
- **`LF_RUN_ID` presence still decides three behaviours** without checking the
  Run: the direct-skill checkpoint in `bin/lf.rs`, branch-Task lookup in
  `ops/task/lifecycle.rs`, and the progress marker on Task comments in
  `ops/linear_observe.rs`. A stale value silently marks a person's comment as
  agent progress, which excludes it from steers. Routing all three through
  `session_record::inherited_caller` would validate against the manifest, but
  `ops/release.rs` and `lf/commands/util.rs` set `LF_RUN_ID` on paths that were
  not traced for a matching `LF_RUN_DIR`. Left as a follow-up; the tmux fix
  removes the known way a stale value arrives.
- **Webhook and daemon names dropped from the scrub list.**
  `LF_LINEAR_WEBHOOK_SECRET`, `LF_LINEAR_VIEWER_ID`, `LF_GITHUB_WEBHOOK_SECRET`,
  `LF_GITHUB_WEBHOOK_URL` and `LF_LFD_ALLOW_NON_LOOPBACK` have no reader. If a
  Doppler config still injects secrets under those names, session shells no
  longer unset them. The Doppler name check returned nothing matching.
- **Installed launchd plists** for cron and CI watch still carry `LF_DB_PATH`
  and `LoopflowDbPath`. Both are ignored; reinstalling the schedules rewrites
  them.
- **Jack's tmux server** still has `LF_USER_NAME`, `LOOPFLOW_DIRECTIVE_FILE`
  and `LOOPFLOW_FLOW_NAME` in its global environment from an earlier client.
  The fix stops new servers inheriting context; it does not clean a running
  one. `tmux set-environment -gu NAME` clears each.
- **In-crate tests still read ambient Exec context.** Run from inside a Task
  session, four lib tests (`lf::commands::run::tests::{ad_hoc_batch_launch…,
  two_task_research_runs…}`, `ops::chapter::tests::{every_provider_mutation…,
  legacy_project_adoption…}`) fail on the inherited `LF_FLOW_STEP`/`LF_RUN_DIR`
  and pass with `LF_*` removed. The integration `EnvGuard` does not cover lib
  tests. Not changed here.
- **`lf sync` had nothing to merge**: the branch base already equals main
  (`3d1f76780`). LOO-341 has no comments.

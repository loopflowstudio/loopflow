# Cut 4 (organ kill) — executive decisions taken headless

- **`runs.activation_log_id` dropped entirely** (column + `Run` field), not
  just the tables: the column carried a FOREIGN KEY into `activation_log`,
  and with the parent table gone sqlite fails to *prepare* any statement
  touching `runs`. Migration 050 drops the column first (drops the FK with
  it), then the tables. The field never crossed the wire (`RunDto` didn't
  carry it), so no mirror churn.
- **The whole runner tree died with the dispatch chains** — docker/ and
  local.rs executors, `AgentExecutor`, launch.rs, summary.rs: everything was
  reachable only from `execute()`/the dead loops. `WaveExecutor` survives as
  {palette sessions, boot session reconcile, worktree janitor}; the
  workspace-file helpers `lf` fork steps use moved to `executor/mod.rs`.
- **`launch_palette_session` kept**: POST /v0/sessions is a live registered
  route (collapse.md flagged it "no live caller" — the route disagrees;
  revisit if Concerto terminals are confirmed dead).
- **Queue-reconcile timer dropped, not re-homed**: `lf op queue reconcile`
  is per-wave, so "one honest line on a timer" wasn't available; the
  PR-merged webhook drives it. `reconcile_attention_items` (which rode the
  same 60s loop) has no caller now — left in place in lfd/attention.rs as a
  pub helper; delete or re-home when attention moves to files.
- **Event variants CiFailure/Activation*/Agent* removed** from the WS wire
  enum (producers all died; Swift decodes leniently and never referenced
  them). WaveStarted/WaveWaiting kept — WaveStopped's siblings, still in
  ws.rs enrichment.
- **Swift keeps its Trigger/WaveCron models + trigger UI paths** (they parse
  leniently to empty and the mutation calls were already ghost-surface);
  only the contract tests changed. The python mirror dropped the models —
  they were wire-required there.

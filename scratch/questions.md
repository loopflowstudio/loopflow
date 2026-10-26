# LOO-614 open questions and assumptions

- Decided (Jack Heart, 2026-10-26, LOO-602 decision 6): no Loopflow transcript copy and no `loopflow` retention state; no provider retention settings synced. Claude's `cleanupPeriodDays` (default 30) is named as the way to keep more; Codex, OpenCode and Pi keep everything (`ExpiryPolicy::Kept`).
- Assumption: `lf history show SESSION --json` keeps today's input snapshot (tests and canonical-id fixtures read it). The conversation goes in the text output and `--transcript [--json]`.
- Assumption: OpenCode Sessions count as `unknown` presence in `lf history retention`, because checking needs `opencode session export` per Session. Their policy is `kept`. `lf history show` reads them exactly. 8 OpenCode Sessions on Jack's machine.
- Assumption: Claude expiry uses `<home>/settings.json` `cleanupPeriodDays` plus the transcript mtime, worded "removed after". Project and managed settings precedence is not modeled.
- Assumption: gone-transcript facts show time, status and final answer per turn, not per-turn tokens. The `show` header already prints context cost; native usage needs the baseline arithmetic `session_record` owns. Reversible.
- Assumption: acceptance proof for show/retention is a lib test over an ephemeral store with isolated account homes (`transcript::summary::tests`), not a CLI integration test; it needs no `CLAUDE_CONFIG_DIR`/`CODEX_HOME` env mutation. CLI smoke ran against Jack's store.
- Assumption: age buckets use transcript mtime when present, else the Session's created/completed time (a per-Session `MAX(observed_at)` scan is ~2.7M rows).
- Fall-out: cross-provider continuation now reads Pi too, through the same reader.
- Open: whether the Desktop Retention card should link each Session to a conversation view. No Desktop transcript view exists today, so it is excluded here.

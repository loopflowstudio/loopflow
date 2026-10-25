# LOO-580: reconnect to Sessions whose terminal is gone

Status: implemented 2026-10-24; realigned 2026-10-24 (no upstream changes to touched files).
Remaining: gate, ship (Jack: fully automatic, no demo review). Open acceptance gap: the
Task's "Done when" asks for a real terminal-closed Codex Session reconnected by the branch
build within seconds, recorded in the PR; only the parts below are proven so far.

- `resolve_target` (ops/human_session.rs): a Task identifier where a Session id belongs
  becomes a Task target in `select_connection` and `explain_connect`; exact Session ids win.
- `sessions_for_agent_session`: receipt prefilter; unknown ids fail in ~1 s, not 26 s.
- `SessionState::Detached` from the transport owner's recorded viewers; terminal ids are
  dropped when no viewer occupies them.

Evidence (Jack's machine, 2026-10-24): `lf session connect NOPE-185` 26 s on 0.14.15 →
0.98 s on the branch build; Session 70ce9350… had owner `viewed=0`, `lf session list`
reported `active` + "Move here"; observe-attach to that owner answered in 0.49 s.
The branch build runs on a stale Machine copy (`~/.lf/copies/45407719…`, in use by
another process), so MTG-185 itself could not be connected by the branch build here.

Check: `cargo test -p loopflow --test integration a_task_identifier_connects` + `--lib human_session` (34) pass; clippy clean. Swift build deferred to gate/CI.

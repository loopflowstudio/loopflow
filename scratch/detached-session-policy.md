# End detached Sessions after a configurable idle period

Task: [LOO-556 · Close windows without leaving agents running for days](https://linear.app/loopflow/issue/LOO-556).
LOO-556 has no Linear Project yet (2026-10-21); the PR links LOO-556 directly.

## Problem

Since #1605 (Keep conversations across window closes), closing a window or
switching Tasks keeps the Session's agent running so it can be reattached.
Nothing ever ends a detached Session, so live agents pile up. On 2026-10-21
Jack Heart's laptop had 40+ interactive claude/codex agents (14 GB RAM) under
detached `transport serve --terminal` owners 1–2½ days old, each also holding
the 9 GB store open.

## Decision (Jack Heart, 2026-10-21)

- A configurable policy, delivered as a PR. The setting is the idle time after a
  Session's last viewer detaches. Default 24 hours. A value of off disables it.
- Measure time since the window closed (last viewer detached), not time since
  start. Record the detach time; it isn't recorded today.
- Sessions from builds that never recorded a detach time fall back to 48 hours
  since start.
- Ending a Session ends its agent process; `lf resume` / Desktop Resume brings
  it back from provider history. `lf resume --all` must not resurrect Sessions
  ended this way.

## Never end

- A Session with a turn in progress. Judge by Session state, not CPU.
- A Session with an attached viewer.
- A Flow step, including Flows started from a terminal.

Counterexample from 2026-10-21: a manual sweep by process age killed agents
inside two terminal-started Task Flows (LOO-532 pursue, MTG-156 code) and two
busy codex agents. The policy must make those cases impossible, with tests.

## Acceptance

- Config key (name tentative) in `.lf/config.yaml` and the user config, default 24h.
- Detach time is recorded when the last viewer leaves; reattaching clears it.
- Something already running periodically applies the policy. Tentatively the
  owning `transport serve` process times itself out, so no new daemon is needed.
- Tests cover: idle past the limit ends; viewer attached, turn in progress and
  Flow step are kept; off disables; legacy 48h fallback; ended Sessions resume
  from history and are skipped by `lf resume --all`.

## Chosen approach (2026-10-21, implemented)

- **Config:** `session.end_detached_after` (`24h` default, `90m`, `2d`, `off`),
  global or repo `.lf/config.yaml`. One duration parser (`config::parse_duration`)
  now serves this, `lf … --timeout` and cron waits.
- **Record:** draft `detached_owner_viewers` adds `processes.viewed` and
  `processes.detached_at` on the execution owner's row. The owner writes
  `viewed=0, detached_at=start` when it starts, `viewed=1` (clearing the time) when
  any viewer attaches, and the time again when the last one leaves. Both NULL
  means a build that never reported viewers → 48h-since-start fallback.
- **Judge:** every new owner runs a thread that, every `limit/12` (1s–5min),
  judges all running owners on the Machine (`idle_session_owners`), so legacy
  owners are covered without a daemon. Eligible owners host only unfinished
  interactive Sessions with no Flow (no Session Flow membership, no Flow process
  descending from the owner) and no turn running (same SQL as `turn_cut_off`).
  An unreadable config keeps that repo's owners.
- **End:** SIGTERM to the owner after a live identity check. Its interrupt path
  kills the agent's group and records the owner's exit (`interrupted`), so
  `resume --all` (which needs an unexited owner) skips it and `lf resume` reopens
  from provider history. The owner now also removes its socket on SIGTERM.

## Alternatives rejected

- Owner self-timeout only: cannot reach the 40+ legacy owners running old code.
- A runtime file beside the socket: avoids a migration but splits Process state
  out of SQLite.
- Process age or CPU as the signal: the 2026-10-21 counterexample.

## Checks

- `cargo test -p loopflow --test transport_tests detached_sessions_end_after_their_limit`: pass
- `cargo test -p loopflow --lib -- transport::idle config::tests::detached cron`: pass
- compress: one owner query (Flow ancestry as a CTE), `platform::kill_process` for the signal; transport test, lib tests and clippy pass

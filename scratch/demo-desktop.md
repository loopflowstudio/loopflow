# Desktop demo on a copy of real data (LOO-718, 2026-11-11)

Branch Desktop at `355d0a092c` on a snapshot of `~/.lf/loopflow.db`, so the
installed revision 4 server and live Sessions stay untouched.

- History root (`LF_HOME`): `/Users/jack/.lf/demos/loo718-desktop/home`:
  store snapshot taken 2026-11-11 15:10 with `sqlite3` backup, plus
  `config.yaml`, `loopflow-state.yaml`, `flows/`, `agents/`. No credentials or
  accounts directories. The branch server migrated the copy
  (`bounded_identity_triggers`).
- Runtime root: `/Users/jack/.lf/demos/loo718-desktop/runtime`: `Loopflow
  Dev.app` built from this checkout; `LF_BIN` is its `Contents/MacOS/lf`. App
  log: `runtime/app.log`.
- Processes: Desktop PID 29029 (`…/runtime/Loopflow Dev.app/Contents/MacOS/Loopflow
  Dev --repo /Users/jack/src/loopflow`); server PID 30349, launchd job
  `com.loopflow.server.d7b2207251ad5550`, socket
  `/private/tmp/lf-501-d7b2207251ad5550/machine.sock`, revision 5.
- Writes made in the demo act on the real repository checkouts and are not saved
  back to the real Machine.
- The bundle ID is `com.loopflow.mac.dev`, so window state and preferences share
  the regular Dev app's defaults domain.

Teardown, once nothing in the demo is pending: quit the app (PID 29029), then
`launchctl bootout gui/501/com.loopflow.server.d7b2207251ad5550`, confirm
PID 30349 exited, and remove the runtime root. Keep the history root. Reopen:
same `env LF_HOME=… LF_BIN=…` launch of the runtime app.

## What happened

Torn down 2026-11-11 23:25 UTC: app, conversations, owners and server ended;
launchd job booted out; runtime root removed; history root kept.

Restoring the saved layout ran two Task shells' `lf --task 4ceb1d ship` and
`lf --task LOO-809 ship` again under the demo Machine. Both were stopped within
about a minute, and restarted once (with the demo server re-registering through
`ensure` while demo processes still asked for it) before the second stop. No
commits or PR changes came from them: LOO-809 has no PR, and manabot PR #462
merged at 23:14:33 UTC, before the demo started. The Sessions opened in the demo
were Jack's own.

A copied store carries panes, commands and Sessions without the live processes
behind them, so a second Machine on a copy restarts real work. A real-data demo
of a protocol bump needs the server itself swapped, or a server that speaks the
previous revision.

## Second demo: the one-task world

`lf-world build one-task` at `355d0a092c` into
`/Users/jack/.lf/demos/loo718-world/world` (its own `LF_HOME`, repo, bare origin,
and stand-in `claude`, `gh` and `launchctl`, so nothing reaches real Tasks,
checkouts, GitHub or launchd). Runtime root `/Users/jack/.lf/demos/loo718-world/runtime`
holds the app; launched with the world's `env.json` plus `LF_BIN` set to the app's `lf`.
Desktop PID 72558; server PID 76195 (socket
`/private/tmp/lf-501-3b3c8889903d631e/machine.sock`, revision 5). Teardown: quit
the app, end PID 76195 after confirming its command line, remove the runtime root.

Desktop showed "repository plan repo_30115cfb3a8ecb51494082029703a1fa has no local
checkout; run `lf repo --bind …`": `portfolioRepos` in the shared
`com.loopflow.mac.dev` defaults remembers the real loopflow repository, which the
world's Machine has never seen. Defaults aren't keyed by `LF_HOME`, and the error
suggests a bind that would point the world at the real checkout.

World demo stopped 2026-11-11 (app and server ended by PID); runtime root still there.

## Third attempt: the real server (not swapped)

Jack Heart: "can we just take over the server?" / "yes".

- Branch merged the installed release commit `d382d0b72` (not later main, whose
  `agent_process_inbox` the real store lacks); its unreleased migrations then
  match `~/.lf/loopflow.db` exactly.
- Uncommitted: `MIN_REVISION = 4` in `server/wire.rs`, so revision 4 clients connect.
  Built into `/Users/jack/.lf/demos/loo718-real/runtime/Loopflow Dev.app`.
- Dev defaults exported to `/Users/jack/.lf/demos/loo718-real/history/com.loopflow.mac.dev.plist`;
  every `warm-conversations-*` key deleted. Restore: `defaults import
  com.loopflow.mac.dev <that file>`.
- Stopped before the swap: with the default `LF_HOME`, `lf install server`
  (Desktop's first server call) execs the selected installation, and its
  `ensure` treats a server on another executable as not ready and re-registers
  its own. Each 0.15.3 managed launch does the same. A bare branch server on the
  default socket gets replaced. Taking over means the branch build becomes the
  selected installation.

Jack Heart: "thats fine with me", then picked "Promote the branch".

- 2026-11-11 19:50 local: `scripts/install.py local`, then
  `local-bin/app.noindex/Loopflow.app/Contents/MacOS/lf install promote
  --cli-target ~/.local/bin/lf --app-source … --app-target /Applications/Loopflow.app`.
  Preflight: nothing to migrate, 22 executable references resolve. Installed lf
  is now `0.15.3+0e420b33b.dirty`; server PID 682 on revision 5.
- `com.loopflow.mac` defaults exported to `history/com.loopflow.mac.plist`; its
  `warm-conversations-*` keys deleted. Loopflow PID 19663 opened on
  `/Users/jack/src/loopflow`. No ship command started after launch (four
  running `ship` processes predate it).
- Back to the release: `lf install --stable`, then `defaults import` both plists.

## What was said

Jack Heart: "i dont think copying sessions works" / "it never has" / "i act
actually open the sessions" / "i dont think they loaded properly".


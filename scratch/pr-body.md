## Try it!

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test -p loopflow
uv run pytest python/tests/test_dto_fixtures.py -q
swift test --package-path swift
uv run python scripts/verify_embedded_build_driver.py --skip-build
```

The verify script starts local `lfd`, creates a throwaway wave, posts to `POST /v0/terminal-sessions`, waits for the launched flow to finish, and checks that the tmux session is still attachable afterward.

## Intent

Make Concerto's embedded terminal a first-class build-driving surface. Palette and launchpad flows now launch into lfd-owned tmux sessions, persist the session id in the multiplexer layout, reattach through the existing terminal-session attach API, and show the selected agent in the pane header.

## Assumptions

- lfd should own embedded build terminals; Swift should attach by session id rather than synthesize tmux session names.
- `TerminalSession.source == "palette"` is the right provenance for stay-alive palette launches for now.
- `TerminalSession.agent` is the provider/model display string and also the value passed to `lf -m`.
- Renaming persisted pane config from `terminalSessionName` to `terminalSessionId` can reset old local terminal bindings once.

## Key decisions

- Added `POST /v0/terminal-sessions` with a required `{wave_id, flow, worktree, agent}` request and a `{session, connection}` response.
- Palette terminal sessions write an exit file, complete the lfd row from that file, then `exec $SHELL` so scrollback and reruns survive after the flow exits.
- Startup reconcile handles tmux sessions that outlive lfd and rows whose tmux sessions disappeared while lfd was down.
- Multiplexer panes store lfd session ids and attach through `RepoState.attachTerminalSession`; the old synthesized `lf-<waveId>-<paneId>` path is gone.
- DTO fixtures pin the new create request and terminal-session response shape across Rust, Swift, and Python.
- Gitignored `.lf/tmp/`: palette sessions write exit files there, so the runtime scratch directory must not be tracked.

## Not included

- Native chat UX.
- A user-shell create endpoint for blank terminal panes.
- Remote embedded tmux attach.
- New `TerminalSession` database fields or migrations.

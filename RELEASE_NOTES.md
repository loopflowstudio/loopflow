# v0.9.1

Loopflow 0.9.1 ships Chords (compose multiple waves into ensembles), a bundled daemon inside Concerto, and a portfolio dashboard that replaces the old welcome screen. Security hardening continues with credential hygiene and API surface gating.

## New capabilities

- **Chords** — compose waves into multi-voice ensembles with `loopflow.join()`, and add cross-wave awareness with `loopflow.add_stimulus("designer", kind="listen", source_wave_id="infra")` (#384, #391)
- **Portfolio dashboard** — Concerto now opens a live dashboard showing every repo's wave status, blocked count, and diff totals instead of the old welcome panel (#401)
- **Bundled daemon** — Concerto ships its own `lfd`; open a repo and the daemon starts automatically on an ephemeral port with a generated token. No external install required (#404)
- **Claude sessions** — `agentapi` supports Claude as a session provider with automatic `--resume` on subsequent turns (#382)
- **Step-aware sessions** — sessions now accept a `step` field (e.g. `"design"`, `"ci-fix"`) instead of raw system prompts, and new built-in steps `lf ci-fix` and `lf release` are available out of the box (#400)
- **Remote editor launch** — in Concerto connected to a remote `lfd`, open a terminal (SSH), Cursor/VSCode (`--remote ssh-remote+host`), or Zed (`ssh://host/path`) directly into the remote worktree (#388)
- **`lf release`** — generates release notes from merged PRs since the last tag and auto-creates a git tag on merge to main (#397)

## Improvements

- **Docker executor supports fork flows** — `lf wave run` with fork-based flows (e.g. `wave-reduce`) now works identically in Docker and native mode (#380)
- **Wave config is just a YAML file** — wave configuration lives at `wave/<name>/<name>.yaml` on disk; the schema discovery abstraction is gone (#387)
- **Python session API client** — `from loopflow.api import create_session, send_session_input` for scripting session lifecycles (#389)
- **`lfq` available in the Docker image** — the queue client is bundled alongside `lf` and `lfd` in the container (#399)
- **Simpler local install** — `python3 scripts/install.py local` replaces the old publish script; DMG building moves to CI (#405)
- **Cleaner Codex integration** — uses the native `--yolo` flag instead of verbose sandbox overrides (#395)

## Security

- **API surface gating** — configurable request-size limits for JSON bodies, WebSocket frames, and hook payloads via `lfd.yaml` or environment variables (#385)
- **Credential hygiene** — secrets are zeroized on drop and redacted from logs; `lfd token rotate` rotates static auth tokens; query-parameter credentials are now rejected with 400 (#392)

## Infrastructure / reliability

- **E2E test harness** — hermetic smoke tests build `lfd` from source against an isolated HOME and temp repo, covering the full wave CRUD lifecycle (#390)
- **PR/commit message validation** — the ops message pipeline now requires structured JSON from the agent and rejects malformed output instead of silently accepting garbage (#403)
- **Legacy Rust agent removed** — the standalone Rust agent loop, tool dispatch, and `lf-agent` binary are gone; `portable-pty` dependency dropped (#406)
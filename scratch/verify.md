# LOO-984 verify: Verified

PR #1991 merged as f76129f677 (2026-11-20T00:47:03Z). This checkout's tree
equals main's (`git rev-parse HEAD^{tree} origin/main^{tree}` → 67503fc344
twice), so `cargo build -p loopflow --bin lf` here is main.

Driven: `lf -a claude resume` under a Python PTY in a fresh repository,
private `LF_HOME`, `LF_*`/`CLAUDE_CODE*` stripped, a stand-in `claude` first
on `PATH`. Screens rendered through pyte; launches read from the private
store. One Session (`80c38d33…`) throughout, 5 captured inputs.

| Open | Stand-in | On screen | Launched with |
|---|---|---|---|
| 1 | exits 1, saves nothing | "Nothing to resume here; starting a new conversation." | `--session-id 0e65407e…` |
| 2 | same | "This conversation has no saved claude transcript; starting it again from its first request." | `--session-id 01e2c12e…` |
| 3 | saves a transcript | start-again line | `--session-id 4afa5b4a…` |
| 4, 5 | refuses `--resume` | "No conversation found with session ID: 4afa5b4a…" | `--resume 4afa5b4a…` both times; no new input |
| 6 | transcript deleted, no `claude` on `PATH` | start-again line, then "Error: 'claude' is not installed. Install it with `npm install -g @anthropic-ai/claude-code`…" | nothing |
| 7 | `claude` back | start-again line, "standin saved 093184ea…" | `--session-id 093184ea…` |

Removed code is gone from `rust/`: no `which_command`,
`record_missing_provider`, `lost_thread` or `fresh-thread:` request id.
`docs/sessions.md:371` carries the line.

Post-merge CI for f76129f677: merge-queue `CI` success; push `CI` and
`macOS on main` were in progress when read, not passed.

Not checked here: real Claude's `--add-dir` "No, exit" on main (qa saw it on
the branch at 8273da9e9b; product code unchanged since); a pane open in
Desktop or Rex; a lost Codex thread.

Follow-up already filed: LOO-1008 (c2ed1c), headless turn to a never-saved
conversation; unstarted.

Cleanup: launchd job `com.loopflow.server.b330e0ea3fd4afff` booted out,
`/tmp/v984` and its `~/.claude/projects` folder removed.

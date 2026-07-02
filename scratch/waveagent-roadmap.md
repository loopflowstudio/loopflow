---
requires: existing code
produces: code changes
---
Replace the on-disk roadmap file-mirror with `lf op roadmap` — a live Asana fetch the
looping agent calls as a tool. Keep the Asana connection; delete the local mirror.

Design decisions (settled):
1. **Nuke the file-mirror, keep connect.** Delete the on-disk roadmap mirror; keep
   Asana auth + project create/resolve.
2. **Agent calls `lf op roadmap`.** lfd does NOT fetch Asana at render time. The loop
   runs `lf op roadmap` itself to pull the current roadmap. The render context's
   roadmap handle is the wave's authored `roadmap:` value (from GOAL.md), not the
   `wave/<name>/` dir path.

## Add — `lf op roadmap`

New `op` subcommand (register in `rust/loopflow/bin/lf.rs` under the `op` command;
implement in `rust/loopflow/src/ops/` — a new `roadmap.rs` or fold into `pm.rs`, your
call, but keep it clean):

- `lf op roadmap [--wave <name>]` — resolve the wave (from `--wave`, else the current
  branch/worktree the way other `lf op` commands do), read its `roadmap:` handle from
  `wave/<name>/GOAL.md` frontmatter (e.g. `asana://<project_id>` or a project id via
  the existing `WavePmConfig`/asana config), fetch the project's tasks via the existing
  `AsanaClient` (`rust/loopflow/src/lfd/pm/asana.rs` — `PmProvider` fetch methods), and
  print a scannable roadmap: task name, status, assignee, id. Read-only.
- `lf op roadmap update [--wave <name>] --title <t> [--notes <n>] [--status <s>]` —
  write-back: create-or-update a task on the wave's Asana project (reuse the
  AsanaClient create/update methods that `pm export`/`push-diff` used). This is the
  agent's write-side ("fold results back to the roadmap").
- If the wave has no Asana `roadmap:` handle, print a clear message and exit non-zero
  (no local fallback — the file mirror is gone).

## Remove — the on-disk roadmap mirror

Delete (keep git history):
- `read_local_roadmap_items` and the local-file roadmap path in
  `rust/loopflow/src/ops/pm.rs`.
- The file-mirror commands: `pm_pull`, `pm_import`, `pm_push_diff`, `pm_sync` (the parts
  that read/write `wave/<name>/*.md` roadmap files). If a whole function's only job is
  the file mirror, remove it and its CLI wiring.
- The `wave/<name>/N-*.md` roadmap files themselves: `wave/goals/1-goal-primitive.md`
  (and any other numbered roadmap files under `wave/`). GOAL.md + MEMORY.md stay.

**KEEP** (this is "keep connect"):
- Asana auth (`lf op auth asana`), the `AsanaClient`, and project create/resolve
  (`pm_init`) — rehome project-create under `lf op roadmap init` if clean, or leave
  `pm init` as-is. Do not break `lf op auth`.

## Repoint the loop's roadmap handle

- `rust/loopflow/src/lfd/executor/wave/mod.rs` `build_wave_run_command`: change
  `roadmap: format!("wave/{}", wave.name())` to the wave's authored `roadmap:` handle
  read from GOAL.md (fall back to an empty/"(none)" string if unset). The operating
  prompt already tells the agent to read the roadmap; with this handle + `lf op roadmap`
  as a tool, the agent fetches live. Do not make build_wave_run_command call Asana.

## Rewire flows/steps (drop the file-mirror steps)

- `build/flow/deploy.yaml`: remove the `op: pm push-diff` item.
- `ops/flow/sync.yaml`: remove the `op: pm pull` item.
- `build/flow/build-or-silent.yaml`: remove the `op: pm pull` item (keep `ingest`/xor).
- `build/step/design.md`: remove the instruction to write `wave/<name>/1-*.md … 4-*.md`
  roadmap files. Design produces the scratch design doc, not roadmap files.
- `govern/step/ingest.md`: pick the next item from `lf op roadmap` output instead of the
  local `wave/<name>/` mirror.

## Tests

- `lf op roadmap` fetch: mock the Asana HTTP layer (side effect), assert it renders
  tasks from a fake project and errors cleanly when no `roadmap:` handle.
- `lf op roadmap update`: asserts it calls the create/update path (mock Asana).
- Removal: `cargo build` + `cargo test -p loopflow` green with the mirror code gone; no
  dangling references to `read_local_roadmap_items` etc.

## Guardrails

- Do NOT delete `lf op auth`, the AsanaClient, or PM provider trait — only the on-disk
  file-mirror. Do NOT touch trigger/cron code. Do NOT delete unrelated functions.
- `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test -p loopflow`,
  `uv run pytest python/tests/` must pass. Update Python `lf op` wrappers / docs that
  reference `pm pull`/`push-diff` roadmap-mirror behavior.
- Keep the diff scoped to roadmap: CLI add + mirror removal + handle repoint + the
  named flow/step edits.

## Output

`lf op roadmap` (fetch) + `lf op roadmap update` (write-back) working against Asana;
the on-disk `wave/<name>/*.md` roadmap mirror and its pm pull/push-diff machinery gone;
the loop points at the live roadmap handle. Tests green.

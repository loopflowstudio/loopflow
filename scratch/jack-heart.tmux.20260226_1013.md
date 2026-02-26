# tmux wave implementation design (branch `jack-heart.tmux.20260226_1013`)

## Why this doc

This branch should deliver a meaningful tmux milestone (~1000 LOC) and avoid auth drift.

Target outcome for this PR:

- ship a real, installable tmux plugin surface
- make the plugin useful in both `lf` mode and container-backed `lfq` mode
- keep scope tight enough to ship confidently in one cycle

This doc is intentionally exhaustive so implementation can proceed with minimal ambiguity.

---

## Scope decision for this branch

Implement **tmux phases 01–03** end-to-end now, with production-level behavior:

1. TPM/plugin skeleton + status plumbing
2. named layouts
3. mode-aware keybindings + picker flows

Defer full lifecycle orchestration from phase 04 (`lfd install/start/stop/...`) to the auth/bundledcontainer branch or follow-up PRs.

### Why this split

- 01–03 is a complete user-visible feature set and fits ~1000 LOC.
- 04 crosses into active container/auth changes already being developed elsewhere.
- This keeps this branch tmux-focused and reduces merge conflict surface.

---

## User-facing milestone definition

After merge, a user should be able to:

1. add `@plugin 'loopflowstudio/loopflow.tmux'` in tmux
2. source plugin
3. see loopflow status in status bar
4. press keybindings to:
   - run actions
   - open logs
   - pick a wave/worktree
   - open layout presets
5. use plugin even when some dependencies are missing (clear message, no silent failure)

---

## Out of scope for this PR

- full container lifecycle command implementation (`lfd install/update/uninstall`)
- auth onboarding workflows
- remote daemon provisioning logic
- telemetry backends / metrics ingestion
- Swift/Concerto integration

---

## Implementation footprint (~1000 LOC target)

| File | Purpose | Est LOC |
|---|---|---:|
| `loopflow.tmux` | plugin entrypoint + option defaults + source hooks | 50 |
| `scripts/helpers.sh` | shared mode detection, command wrappers, picker helpers, cache helpers | 260 |
| `scripts/loopflow-status.sh` | status renderer + cache read/write + fallback logic | 180 |
| `scripts/keybindings.sh` | binding registration + dispatch table + UX messages | 220 |
| `scripts/layouts/lf-dev.sh` | single-wave layout | 90 |
| `scripts/layouts/lf-swarm.sh` | swarm layout | 110 |
| `scripts/layouts/lf-flow.sh` | flow-monitor layout | 95 |
| `scripts/tmux-review.py` | one-command manual verification launcher/checklist | 140 |
| `wave/tmux/*.md` + README touchups | docs and usage details | 80 |
| **Total** |  | **1225** |

If we need to stay closer to 1000, trim helper abstraction depth and script comments.

---

## Architecture

### 1) Plugin bootstrap (`loopflow.tmux`)

Responsibilities:

- resolve `CURRENT_DIR`
- set tmux defaults (only if unset):
  - `@loopflow_mode=auto`
  - `@loopflow_key_prefix=l`
  - `@loopflow_status_ttl_ms=2000`
  - `@loopflow_status_timeout_ms=250`
- register status interpolation:
  - `@loopflow_status_format`
  - append `#(scripts/loopflow-status.sh)` into `status-right` safely
- source keybindings script
- idempotent on repeated `source-file`

Non-responsibilities:

- no heavy subprocess calls
- no container start side-effects

---

### 2) Shared helpers (`scripts/helpers.sh`)

Core API (shell functions):

- `loopflow_has_cmd <cmd>`
- `loopflow_mode` -> `lf` | `container`
- `loopflow_mode_explicit` (reads tmux option)
- `loopflow_detect_container_mode`
- `loopflow_display <message>`
- `loopflow_timeout_ms` / `loopflow_status_ttl_ms`
- `loopflow_pick_with_fzf <items...>` (if present)
- `loopflow_pick_with_tmux_menu <items...>`
- `loopflow_pick_wave`
- `loopflow_pick_layout`
- `loopflow_dispatch <action>`
- `loopflow_run_in_pane <cmd>`

Design rules:

- all command strings centralized here or in one command map section
- no unquoted eval of picker output
- every function returns explicit success/failure code

---

### 3) Status renderer (`scripts/loopflow-status.sh`)

Output goals:

- compact string (`[lf: ...]`)
- mode marker implicit in content
- stable render under dependency failures

Data sources by mode:

- `lf` mode:
  - git branch in current pane path
  - optional active `lf` process hint
- `container` mode:
  - `lfq status --json` / `lfq list --json` (whichever is cheaper and stable)

Cache contract:

- file: `/tmp/loopflow-status-$USER.json`
- fields:
  - `generated_at`
  - `mode`
  - `text`
  - `source`
- TTL default 2s
- stale read allowed when live query fails

Failure behavior:

- missing `lf`/`lfq`: `[lf: --]`
- parse failure: fallback cached or minimal token
- timeout/failure: stale cache + marker

Performance:

- hot path should be cached
- avoid spawning more than one expensive process per tick

---

### 4) Layout scripts (`scripts/layouts/*.sh`)

Common contract:

- create new tmux window (never mutate current window)
- use current pane path as cwd unless explicit arg passed
- detect mode once
- seed pane commands with `send-keys`
- for narrow terminals fallback to simplified 2-pane layout

Layouts:

- `lf-dev`: editor + agent + shell
- `lf-swarm`: leader + 3 worker panes
- `lf-flow`: status pane + flow pane + shell

Window names:

- `lf-dev`, `lf-swarm`, `lf-flow`

---

### 5) Keybindings (`scripts/keybindings.sh`)

Binding pattern:

- prefix key from `@loopflow_key_prefix`
- use idempotent unbind/rebind on load
- route all actions through `loopflow_dispatch`

Actions to support:

- `r` run
- `s` stop
- `o` logs
- `p` PR
- `n` next
- `d` land
- `w` wave picker
- `L` layout picker
- `?` help overlay

Feedback contract:

Every action must display one of:

- launched command
- dependency missing
- nothing selected
- mode unavailable

No silent no-op.

---

## Mode semantics

`@loopflow_mode` handling:

- explicit `lf` -> always lf mode
- explicit `container` -> try container commands; fail-soft if unavailable
- `auto`:
  1. if `lfq` missing -> lf
  2. if `lfq status` succeeds -> container
  3. else -> lf

Important:

- plugin load must never start daemon/container
- mode resolution may happen per action, but with short-circuit caching to avoid lag

---

## Security / safety constraints

- never print tokens/secrets in status text
- no shell interpolation of untrusted picker text
- quote all user/path values
- avoid destructive commands in keybindings
- if command includes branch/wave names, pass as positional arguments safely

---

## Compatibility targets

- tmux: 3.2+ baseline
- shell: bash/zsh compatible scripts with bash shebang
- dependencies:
  - required: tmux
  - optional: `lf`, `lfq`, `fzf`, `gh`

---

## Manual verification strategy (required one-command script)

Add `scripts/tmux-review.py`:

Responsibilities:

- print a short walkthrough checklist
- create isolated tmux server socket
- source `loopflow.tmux`
- run read-only checks:
  - status interpolation renders
  - keybindings exist
  - layout scripts execute and create windows
- print commands for interactive follow-up

Run:

```bash
uv run python scripts/tmux-review.py
```

Checklist shown by script:

1. verify status token changes between lf/container availability conditions
2. press `prefix+l+?` and verify help
3. open each layout and confirm pane arrangement
4. trigger picker without fzf and verify fallback
5. kill lfq availability and verify fail-soft messages

---

## Test plan

### Automated-ish (script-backed)

- `scripts/tmux-review.py` for environment bring-up and structural checks

### Manual checks

1. **No deps** (`lf`,`lfq` absent): plugin loads, status fallback, no crashes
2. **lf only**: run/log/layout actions work in lf mode
3. **lfq available + daemon up**: container mode dispatch engages
4. **fzf missing**: wave/layout picker uses fallback
5. **small terminal** (<120 cols): simplified layouts
6. **re-source plugin**: no duplicate binding behavior

---

## Execution plan in this branch

1. **Scope cleanup commit**
   - ensure branch contains only tmux wave/docs/code intent
2. **Implement skeleton + helpers**
   - `loopflow.tmux`
   - `scripts/helpers.sh`
   - `scripts/loopflow-status.sh`
3. **Implement layouts**
   - `scripts/layouts/*.sh`
4. **Implement keybindings**
   - `scripts/keybindings.sh`
5. **Add verification script**
   - `scripts/tmux-review.py`
6. **Docs polish**
   - tmux README + usage snippets
7. **Gate**
   - run manual verification script
   - confirm checklist outcomes

---

## Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| status command latency | sluggish tmux UI | cache + bounded query strategy |
| mode mis-detection | wrong command routing | explicit override + robust auto fallback |
| picker dep missing | broken key workflows | fallback picker path |
| layout fragility on small terminals | bad UX | minimum geometry fallback layout |
| command drift between bindings/layouts | maintenance pain | centralize command templates |

---

## Open questions (to resolve during implement)

1. Do we want layout scripts under `scripts/layouts/` (current plan) or namespaced `scripts/tmux/layouts/`?
2. For container mode, should `run` action default to selected wave or current repo wave inference?
3. Should help overlay use `display-popup` when available, fallback to `display-message`?
4. For status in lf mode, do we show only git branch or also active step/process if detectable?

---

## Definition of done for this PR

- plugin can be installed and sourced via tmux
- status renders reliably with fallback behavior
- layouts open and are usable
- keybindings route actions in both modes (or fail-soft)
- review script exists and can drive manual validation in one command
- docs accurately describe behavior and failure modes


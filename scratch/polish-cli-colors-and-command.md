# Polish CLI colors and command blocks in Desktop (LOO-381)

Status: kickoff plan, implemented on the branch 2026-10-05; not demoed. Jack
Heart requested the Task on 2026-10-05; nothing below is approved by Jack beyond
the Task's own scope and acceptance. Header contents, spacing and prompt
ownership are kickoff choices and stay open to his demo review. Assumptions are
in `scratch/questions.md`. What is left is under Remaining.

## Problem

Embedded terminals in Desktop read worse than Warp in four ways Jack reported:
provider CLIs lose their colors, shell text sits against the left edge with a
repeated `user@host` prompt and no block header, a failed command looks like a
successful one, and text selection and block selection highlight at once.

## Demo

From an agent shell (the tainted case), run `lf desktop`, then in Desktop:

1. Open a Codex Session. The footer shows the model in warm yellow and the path
   in green, as in Warp with the same Codex version and `~/.codex/config.toml`.
2. Open a shell. Run `ls`, then `sdl`. Each command sits under a one-line
   context header (directory, branch), indented off the edge. Only `sdl` has a
   red block background. The fresh prompt below is undecorated.
3. Click the `ls` block, Command-C, paste: `ls` and its output. Drag across two
   words in the `sdl` output: the block highlight disappears; Command-C pastes
   those words. Click the `sdl` block: the text highlight disappears. Narrow the
   window until output wraps and scroll: one highlight, and paste still matches it.

## Findings

**Colors: cause established.** Desktop inherits its launcher's environment, and
the running `/Applications/Loopflow.app` (started 2026-10-05 01:41, parent
launchd) carries `NO_COLOR=1`, `CODEX_CI=1`, `PAGER=cat`, `GIT_PAGER=cat` and
`TERM_PROGRAM=WarpTerminal`: it was opened from a Codex agent's shell, and
`open -a` passes the caller's environment through. `GUIProcessEnvironment.enriched`
strips Loopflow execution markers only.

Codex 0.160.0 footer bytes, captured in a PTY with Jack's config
(`status_line_use_colors = true`), same checkout:

| Environment | Footer |
| --- | --- |
| `TERM=xterm-256color COLORTERM=truecolor TERM_PROGRAM=ghostty` | model `38;2;246;226;183`, path `38;2;171;223;167` |
| same, `TERM_PROGRAM=WarpTerminal` | identical bytes |
| `TERM=xterm-256color` only | 256-color indices 223 / 151 |
| Ghostty environment plus `NO_COLOR=1` | no color sequences at all |
| `TERM=screen`, no `COLORTERM` | no color sequences at all |

Ruled out: Loopflow's muted ANSI palette (Codex emits explicit RGB, so the
palette does not touch the footer); Jack's Ghostty config (template only, no
active lines); Ghostty's launch path (it sets `TERM`, `COLORTERM=truecolor` and
`TERM_PROGRAM` itself); tmux (3.7c defaults to `tmux-256color`, which colors).

Limit: `NO_COLOR` was observed in the app's launch environment and reproduces
the symptom, but was not read from inside a Desktop surface child (`ps` exposes
nothing behind `/usr/bin/login`). The first implementation check closes that.

**Blocks: what exists.** `0001-command-blocks.patch` exposes completed blocks
as `(id, start_row, end_row)` and a read-by-row text call. Swift draws an
overlay and keeps `selectedCommandBlock: (id, text)`.

- A plain click in a completed block never reaches Ghostty, and drags there are
  swallowed. Ghostty's text selection is therefore neither started nor cleared:
  this is the competing highlight, and text inside a completed block cannot be
  selected without a modifier.
- `id` is `node pointer ^ y ^ x`. It changes on reflow and only visible blocks
  are listed, so a selection silently drops on wrap or when scrolled away.
  Copy pastes a snapshot taken at click time.
- Every resting block has a white 4.5% fill: the "shaded `ls` area".
- Exit status reaches the embedder only as a `command_finished` action. Nothing
  per block survives, so a failure cannot be drawn after scroll or reflow.
- Frame math assumes a centered grid. Ghostty's default is
  `window-padding-balance = false` with 2pt top-left padding, so overlays can
  sit up to half a cell off the rows.
- The block list calls `selectCommandBlock` once per viewport row, each walking
  up to the prior prompt, at 10 Hz under the renderer mutex.
- The whole `GhosttyTerminalInputTests` suite is `.requiresDisplay`
  (`LOOPFLOW_NATIVE_TESTS=1`), including tests that need no display. Behind that
  gate the zsh-hook test had been failing since #1356 (`--mode interactive` is
  not a zsh option). Layout, style and zsh-hook tests now live in the ungated
  `GhosttyShellBlockTests`.

**Mechanisms, verified at the pinned `4c838723`.** `Row` is
`packed struct(u64)` with 23 spare bits and reflow copies `semantic_prompt` row
to row, so per-row command status is a small addition. `Screen` tracked pins
survive reflow, and are marked garbage when their rows are evicted or reset. The
zsh integration marks multi-line prompts (`133;A;k=s`) and emits `133;D;<status>`
(no code after a bare Enter).

**Found during implementation: a wrapped command line split its block.** Reflow
copied `semantic_prompt = .prompt` onto the row a soft-wrap creates, so upstream
treats a wrapped prompt or command line as two prompts, and unwrapping let a
continuation row overwrite the first row's metadata. The patch makes the wrapped
row a prompt continuation and keeps the first row's metadata on unwrap. One
upstream test that asserted the old result is changed in the patch.

## Approach

### 1. Terminals do not inherit the launcher's terminal

`GUIProcessEnvironment.enriched` also drops the launcher's terminal session and
agent output policy: `NO_COLOR`, `FORCE_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`,
`COLORTERM`, `TERM`, `TERM_PROGRAM`, `TERM_PROGRAM_VERSION`, `TMUX`,
`TMUX_PANE`, `CI`, `PAGER`, `GIT_PAGER`, `GH_PAGER`, `AI_AGENT`, `CLAUDECODE`,
and the `WARP_`, `CLAUDE_CODE_` and `CODEX_` families except `CODEX_HOME`
(account authority, owned elsewhere). One list, one place, covering `lf desktop`,
the dev launcher and a hand-typed `open`.

Rejected: rebuilding the environment by asking a login shell for its `env`.
It would also retire `enrichedPath`, but costs hundreds of milliseconds against
LOO-376's 400/1000 ms launch targets and depends on interactive rc files.
Rejected: a clean environment only in `lf desktop`; other launchers stay tainted.

### 2. GhosttyKit patch `lf2`: the terminal owns block status and selection

- Record exit status on the block's prompt row when `133;D` arrives, using spare
  `Row` bits (known flag plus status), copied through reflow beside
  `semantic_prompt`.
- `Screen` owns the selected block as a tracked pin. `select(non-null)` clears
  it; selecting a block clears the text selection. Both cannot exist.
- C API: `ghostty_command_block_s { start_row, end_row, exit_code (-1 unknown),
  selected }`; `ghostty_surface_select_command_block(row)` (false and cleared on
  the live prompt or outside a block); `ghostty_surface_read_selected_command_block`.
  `id` and read-by-row are removed.
- Enumerate blocks with one upward prompt search and a forward prompt walk
  across the viewport.
- `ghostty-build` runs the patch's Zig tests before building and fails on them.

The tracked pin held at the pinned revision; no contingency was needed. The
first status reported for a prompt row wins, so `clear` followed by `133;D`
cannot rewrite an older command.

Rejected: Swift pairing `command_finished` actions with block ids (ids do not
survive reflow or scroll). Rejected: making the block a native rectangle
selection (Ghostty copy bindings would copy the header, and the highlight would
fight the failure tint). Rejected: Swift-side exclusion by event ordering only
(keyboard-made selections bypass it).

A new artifact name and checksum go in `Package.swift`. Uploading
`GhosttyKit-4c83872-lf2.xcframework.zip` is additive, but publication has been a
separately authorized action; see `scratch/questions.md`.

### 3. Swift: one selection, honest geometry, calmer blocks

- Pointer events always reach Ghostty. On an unmodified left mouse-up with no
  drag and no text selection, ask Ghostty to select the block at that row.
- Copy reads the text selection if one exists, otherwise the selected block, at
  copy time. No snapshot.
- Rendering comes only from the block list: no resting fill; a hairline above
  each block; hover faint; selected burgundy tint and accent; failed
  (`exit_code > 0`) red tint and red accent, also when selected.
- `embeddedConfig` sets `window-padding-x = 12`, `window-padding-y = 8`,
  `window-padding-balance = false`. The same constants feed the frame math. The
  accent bar sits in the left gutter, clear of text. Padding applies to provider
  panes too; block overlays stay shell-only.

### 4. A Loopflow prompt for zsh shells that still have the system default

A Loopflow-owned zsh `.zshenv`, written to a temporary `ZDOTDIR` at each shell
launch and chained in front of Ghostty's integration, replaces `PROMPT` on first
`precmd` only when it equals macOS's default (`%n@%m %1~ %# `): line one is the
context header (`~/src/loopflow` dim, branch in the accent color), line two is
`❯ `. It turns on `prompt_subst` in that shell so the branch stays current. A customized prompt is left alone and
serves as its own header. Copy excludes the header because the block text is
semantic input plus output. bash and fish keep their prompts.

Rejected: drawing headers in the overlay (needs per-block cwd storage and a
reserved row). Rejected: a settings toggle; adapting to the prompt found needs none.

## Contract

**Source of truth.** Terminal state in Ghostty: rows carry prompt marks and
exit status; `Screen` holds the one text or block selection. Swift derives
overlay and copy from it and holds only hover.

**Consumers.** `GhosttyTerminalView.swift`, `GhosttyManager.swift`,
`ProcessEnvironment.swift`, `buildWorkspaceShellCommand`, `Package.swift`,
`scripts/loopflow-dev.py`, `swift/README.md`, `swift/GhosttyKitPatches/README.md`.
No `lf` wire type changes.

**Absent and error states.** No shell integration (macOS `/bin/bash`, hooks
removed): no blocks, plain terminal, text selection and copy work. A command
with no reported status (`133;D` without a code, or interrupted): neutral, never
red. Selected block evicted from scrollback: selection gone, Command-C copies
nothing. Missing prompt script: the user's prompt, blocks still drawn.

**Operational boundary.** One block query per 100 ms per visible shell pane,
with a single upward prompt search. No launch-time subprocess is added.

**Exclusions.** Xcode/SwiftPM build parity and resource regeneration (LOO-280);
client provenance and shared viewing (LOO-282, LOO-283); live workspace and
installation cleanup (LOO-380); palette changes; exit-code or duration badges;
per-part copy actions; Loopflow prompts for bash and fish; the tmux server's
retained environment for Sessions first started outside Desktop.

**Delete, do not maintain.** `GhosttyCommandBlockLayout.id`;
`selectedCommandBlock`, `commandBlockMouseDown`, `readCommandBlock`,
`clearCommandBlockSelection`; the centered-grid inset in `ghosttyViewportRow`
and `ghosttyCommandBlockFrame`; the resting fill; `ghostty_surface_read_command_block`
and the pointer-xor id in the patch; `ghosttyCommandBlock(atViewportRow:in:)` and
its `commandBlockHitTesting` test (Ghostty resolves the clicked block; its Zig
tests cover that). All are gone. `commandBlockClickAndCopy` and
`commandBlockLayout` are rewritten against the new behavior, not kept beside it.

**Forbidden outcomes.** A Swift copy of selection state or exit status beside
Ghostty's; a second patch file layered on `0001`; a committed local-path
`binaryTarget`; block overlays on `.session` terminals; a failure color inferred
from output text.

## Remaining

- **Publish `GhosttyKit-4c83872-lf2.xcframework.zip`.** `Package.swift` pins its
  URL and checksum (`92f1b61c…`); until the file is at `bin.loopflow.studio`,
  no checkout can resolve the package, so `swift build`, the gate and the demo
  build all wait on it. The built zip is in `swift/.build/artifacts/` (copy in
  `swift/.build/local/`) on the machine that ran `ghostty-build`. Publication
  was not authorized in this pass (`scratch/questions.md` 1). Until then, check
  locally by swapping the pin for `path: ".build/local/GhosttyKit.xcframework"`
  and reverting it before committing.
- **Display checks.** The real-PTY suite (`LOOPFLOW_NATIVE_TESTS=1`) and the
  demo walk-through have not run; overlay rows have not been compared with
  rendered rows on screen.
- **Color proof in a Desktop pane.** The environment fix is tested on the
  launch environment Desktop computes; the Codex footer has not been looked at inside Desktop.
- `swift/project.yml` (Xcode build) still links Ghostty-disabled stubs: LOO-280.

## Done when

- A Codex Session opened in a Desktop launched from an agent shell shows the
  colored footer; `env` in a Desktop shell there shows no `NO_COLOR`, `PAGER=cat`
  or `CODEX_CI`, and shows `COLORTERM=truecolor`.
- In a Desktop zsh shell, `ls` then `sdl`: only `sdl` is red, each has a header
  and gutter, and the live prompt has neither.
- The demo's selection sequence never shows two highlights, and every paste
  equals the highlighted selection, through scroll and a wrapping resize.
- Provider panes show no block overlays; typing, paste, image paste and retained
  Session switching behave as before.

Gate, headless:

- `cd swift && swift build` succeeds.
- `cd swift && swift test --filter "GhosttyShellBlockTests|LocalWaveAgentLauncherTests"`
  passes (environment, frame math against padding, block style, zsh hooks, prompt
  script emitting `133;A`, `133;A;k=s`, `133;B` and `133;D;127` for a missing command).
- `uv run python scripts/loopflow-dev.py ghostty-build` passes its Zig tests and
  prints the checksum pinned in `Package.swift`.

Display diagnostic, not a gate: `LOOPFLOW_NATIVE_TESTS=1 swift test --filter
"Embedded terminal"` drives a real PTY through `true`, `false`, click, drag,
click, resize and Command-C, and checks overlay rows against rendered rows.
Interaction acceptance is Jack's demo review.

## Risks

- A leaked name missing from the list reappears as a different symptom. The
  launch-environment test names the category; additions stay in one array.
- Overriding even the default prompt may be unwelcome; the header is the part
  most likely to change at demo.
- Padding on provider TUIs changes their column count by about three cells.

Check: against the local `lf2` framework (pin swapped to a path, then restored),
`swift build` and `swift test --filter "GhosttyShellBlockTests|LocalWaveAgentLauncherTests"`
— 13 passed, three runs. Display suite and Zig tests not rerun; gate owns them.

Check: kickoff PTY capture of the Codex 0.160.0 footer per environment (throwaway
script, not in the repository) — colored under Ghostty and Warp environments,
colorless with `NO_COLOR=1`.

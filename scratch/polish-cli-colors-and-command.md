# Polish CLI colors and command blocks in Desktop (LOO-381)

Status: kickoff plan, implemented on the branch 2026-10-05. Jack saw the first
build that day; the header pass that followed his feedback is unseen. Jack
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
   context header (the directory), indented off the edge. Only `sdl` has a
   red block background. The header is smaller and dimmer than the bold
   command. The fresh prompt below has its header and no block chrome.
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
and the `WARP_`, `CLAUDE_CODE_` and `CODEX_` families except the provider
account variables `lf` reads (`CODEX_HOME`, `CODEX_ACCESS_TOKEN`,
`CODEX_API_KEY`, `CLAUDE_CODE_OAUTH_TOKEN`: account authority, owned elsewhere). One list, one place, covering `lf desktop`,
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

Revised 2026-10-05 after Jack's demo feedback (directory, spacing, a header in
its own size and color; git and PR "not needed per se"). Details in
[the demo record](polish-cli-colors-and-command-demo.md).

A Loopflow-owned zsh `.zshenv`, written to a temporary `ZDOTDIR` at each shell
launch and chained in front of Ghostty's integration, replaces `PROMPT` on first
`precmd` only when it equals macOS's default (`%n@%m %1~ %# `). The prompt is
three rows: a blank row, a header row, and `❯ ` with the command in bold
(`zle_highlight`, which zsh ends before output).

The header row is the directory as concealed terminal text behind a marker
(`¶ `), truncated to 60 characters. The terminal therefore still owns it: it is
the directory at submission and scrolls and reflows with its rows. The overlay
reads marked rows back with Ghostty's existing `ghostty_surface_read_text` and
draws them at 11pt in dim grey. No patch change and no per-block storage.

Block bands move half a row down on each edge that borders a blank row, so the
gap is shared by the blocks on either side and a failed block is red evenly.

A customized prompt is left alone, serves as its own header, and gets no blank
row or shift. Copy excludes the header because block text is semantic input
plus output. bash and fish keep their prompts.

Rejected: recording the directory on the prompt row in the patch (a new
artifact for what a terminal row already stores). Rejected: one rectangle read
of the viewport (Ghostty joins soft-wrapped rows, so lines stop matching rows).
Rejected: a trailing `%B` in the prompt (output stays bold). Rejected: a
settings toggle; adapting to the prompt found needs none.

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

**Operational boundary.** Per visible shell pane every 100 ms: one block query
with a single upward prompt search, plus one short row read for each row that
can hold a header (a block's first two rows and rows outside completed blocks).
No launch-time subprocess is added.

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
- **Display checks.** The real-PTY suite (`LOOPFLOW_NATIVE_TESTS=1`) has not
  run; overlay rows have not been compared with rendered rows on screen.
- **The overlay header has not been seen.** Headless tests cover the prompt
  bytes, marker parsing, candidate rows and frames. That Ghostty hides the
  concealed row and that the row read returns it rest on reading Ghostty's
  source at the pinned revision. If either fails on screen, a shell shows the
  header twice or not at all.
- **Jack's open points:** whether the gap is right, what "the bar on the
  right" is, and which Warp affordances come next (`scratch/questions.md` 7–10).
- **The polish pass is unseen and its display checks have not run**; details
  under Further polish.
- **Color proof in a Desktop pane.** The environment fix is tested on the
  launch environment Desktop computes; the Codex footer has not been looked at inside Desktop.
- `swift/project.yml` (Xcode build) still links Ghostty-disabled stubs: LOO-280.

## Further polish (built 2026-10-05, unseen)

Jack Heart asked on 2026-10-05 for the terminal research
([terminal-ux-research.md](terminal-ux-research.md)) to be turned into
recommendations, then for the five below to be built. The research is Hacker
News-heavy and not from Loopflow users. Jack has seen none of this on screen.

1. **Right-click keeps the selection it is over.** A right-click on the selected
   block opens the menu without telling Ghostty, whose press would select the
   word there and end the block. Elsewhere the press goes to Ghostty at its own
   position; Ghostty (`Surface.zig`, `right-click-action = context-menu`) keeps
   a text selection that contains the press and otherwise selects the word.
   The menu now opens at the pointer (it converted the point the wrong way).
   Not in the display suite: the menu is modal, so a test calling
   `rightMouseDown` would block.
2. **A selected block ends on the first key that goes to the shell**, Escape
   included, and on paste or drop. Escape still reaches the shell, as it does
   when Ghostty clears a text selection. Command chords (copy, jump) keep it.
3. **Command-Up and Command-Down jump between prompts.** Upstream already binds
   them by default; `embeddedConfig` states both so a person's Ghostty config
   cannot rebind them. The config is global, so provider panes have the binding
   too and, having no prompts, do nothing with it.
4. **Dim text on the fills.** Palette 8 (`#7F766F`) fell below 3:1 on two red
   fills. Red is now 0.14 resting (as Jack saw it), 0.16 hovered, 0.18 selected.

   | Fill | Was | Now | Palette 8 | Header `#A39B93` | Foreground |
   | --- | --- | --- | --- | --- | --- |
   | none `#24211F` | | | 3.60 | 5.84 | 13.03 |
   | hover, burgundy | 0.12 | 0.12 | 3.44 | 5.58 | 12.45 |
   | selected, burgundy | 0.28 | 0.28 | 3.21 | 5.22 | 11.63 |
   | failed | 0.14 | 0.14 | 3.15 | 5.12 | 11.41 |
   | failed, hovered | 0.20 (2.95) | 0.16 | 3.09 | 5.01 | 11.17 |
   | failed, selected | 0.30 (2.61) | 0.18 | 3.02 | 4.90 | 10.92 |

   Cost: a selected failed block differs from an unselected one by a small
   step in red plus the accent going from 0.75 to full. A deeper red at higher
   alpha (`#A82A24` at 0.28, 3.05:1) would separate them more; not chosen.
   Ghostty's own text selection (`#4A443F`) puts palette 8 at 2.16:1; that
   predates this Task and is untouched.
5. **Line editing.** Decided from source, not from a pane:

   | Key | Route in the view | Ghostty sends |
   | --- | --- | --- |
   | Option-Backspace | `interpretKeyEvents` finds no text, key sent with Alt | `ESC DEL` |
   | Option-Left / Right | same | default binding `ESC b` / `ESC f` |
   | Control-A/E/U/W, Control-X Control-E | sent as keys, no text | the control bytes |
   | Command-Backspace, Command-Left / Right | key equivalent, forwarded | default bindings `^U`, `^A`, `^E` |

   The view intercepts only Command-C and Command-V. One probable difference,
   outside the listed keys and not changed: Option with a letter (Option-B,
   Option-F, Option-D) goes through `interpretKeyEvents` and is inserted as the
   composed character (`∫`), where standalone Ghostty on a US layout treats
   Option as Alt and sends `ESC b`. See `questions.md` 12.

Left out, with the reason: sharing, filtering, collapse, bookmarks and sticky
headers (marketed, no first-hand praise found); a Loopflow input editor (Warp's
losses came from replacing the shell's line editor); more header content (Jack
asked for the directory only); per-part copy (revisit after acceptance).

Still to do for these five: Jack's look at each; the display suite, whose
click sequence now also presses a key and Escape, has not run; the per-key
table from a real Desktop zsh pane beside standalone Ghostty; a decision on
Option-letter.

## Done when

- A Codex Session opened in a Desktop launched from an agent shell shows the
  colored footer; `env` in a Desktop shell there shows no `NO_COLOR`, `PAGER=cat`
  or `CODEX_CI`, and shows `COLORTERM=truecolor`.
- In a Desktop zsh shell, `ls` then `sdl`: only `sdl` is red, evenly above and
  below; each has one small dim directory header (never doubled, never
  missing), as does the live prompt.
- The demo's selection sequence never shows two highlights, and every paste
  equals the highlighted selection, through scroll and a wrapping resize.
- Provider panes show no block overlays; typing, paste, image paste and retained
  Session switching behave as before.

Gate, headless:

- `cd swift && swift build` succeeds.
- `cd swift && swift test --filter "GhosttyShellBlockTests|LocalWaveAgentLauncherTests"`
  passes (environment, frame math against padding, block style, zsh hooks, prompt
  script emitting `133;A`, `133;A;k=s`, the concealed header, `133;B` and
  `133;D;127` for a missing command; header parsing and half-row frames).
- `uv run python scripts/loopflow-dev.py ghostty-build` passes its Zig tests and
  prints the checksum pinned in `Package.swift`.

Display diagnostic, not a gate: `LOOPFLOW_NATIVE_TESTS=1 swift test --filter
GhosttyTerminalInputTests` drives a real PTY through `true`, `false`, click, drag,
click, resize and Command-C, and checks overlay rows against rendered rows.
Interaction acceptance is Jack's demo review.

## Risks

- A leaked name missing from the list reappears as a different symptom. The
  launch-environment test names the category; additions stay in one array.
- Overriding even the default prompt may be unwelcome; the header is the part
  most likely to change at demo.
- Padding on provider TUIs changes their column count by about three cells.

Realigned 2026-10-05, twice. First: the prefix scrub also removed provider
account variables `lf` reads; the four are kept and the launch test names them
(a source search finds no other `CODEX_`/`CLAUDE_CODE_` name read outside tests).
Second, after the header pass: the code matches this plan (padding 12/8, marker,
60-character directory, 11pt `#A39B93`, bold command, ungated block suite). The
Demo step still named the branch in the header, and the display filter used the
suite's display name, which `swift test --filter` does not match; both corrected.
`main` has not moved since the base, and the `lf2` URL still returns 404.

Check: against the local `lf2` framework (pin swapped to a path, then restored),
`swift test --filter "GhosttyShellBlockTests|LocalWaveAgentLauncherTests"` —
13 passed after compression (the two zsh hook tests are one; header constants and
terminal text reads each have one owner). Display suite and Zig tests not rerun;
gate owns them. The Zig patch was left alone: any edit needs a new artifact.

Check: kickoff PTY capture of the Codex 0.160.0 footer per environment (throwaway
script, not in the repository) — colored under Ghostty and Warp environments,
colorless with `NO_COLOR=1`.

Check: after the polish pass and its compression, same filter against the local framework — 16 passed (adds fill contrast, right-click, embedded config parsed by Ghostty; the Command-chord rule is inline, covered only by the display suite). Pin restored.

Realigned 2026-10-05 after the polish pass: the five items match this plan in
source (right-click, key and paste/drop clearing, both keybinds, the three red
alphas, the README). The context menu's Clear sends `clear` without ending a
block selection first; the cleared screen drops it anyway. `main` has not
moved; the `lf2` URL still returns 404. Product memory now carries the polish
lessons.

Check: realign by source inspection and `lf context --skill realign` — memory and scratch in budget; no code changed, no tests rerun (gate owns them).

# Demo review: CLI colors and command blocks (LOO-381)

Reviewed 2026-10-05 by Jack Heart in Loopflow Dev, built from this branch at
`de0dac604` against the local `lf2` GhosttyKit framework. Design:
[polish-cli-colors-and-command.md](polish-cli-colors-and-command.md).

## What ran

The app was launched from a Claude Code agent shell in Warp with the variables
from Jack's original report added by hand (`NO_COLOR=1`, `CODEX_CI=1`,
`PAGER=cat`, `GIT_PAGER=cat`; `TERM_PROGRAM=WarpTerminal`, `CLAUDECODE=1` and
`AI_AGENT` came from the shell). `ps eww` on the app process confirmed all of
them were in its environment. The `lf2` artifact is still unpublished (404), so
`Package.swift` pointed at `.build/local/GhosttyKit.xcframework` for the build
only; the committed pin is unchanged. The local zip's checksum equals the pin.

Jack was given the four-part walk-through from the design's Demo section:
colors, blocks, selection, provider panes.

## What Jack said

- "ok looks good but still need a little more space around the cmd blocks".
  Asked where, Jack chose **between blocks** (not inside the block, not the
  left/right inset).
- "take note of all the affordances Warp does".
- "theres teh bar on the right. the header with watever was in the prefix when
  it was sent".

- "we dont need the PR and git in there per se but the cwd and the bar and the
  spacing are all nice", "as well as the different font sizes to differentiate
  header and text", "and colors". These describe what Jack likes in Warp's
  blocks.

Jack did not report on the walk-through step by step. "Looks good" is an
overall verdict on what he looked at; it does not establish that he checked the
Codex footer, the `env` output, each selection sequence, the wrap/scroll case
or the provider pane individually. Those stay unconfirmed.

## Changed during the demo

`LoopflowZshBootstrap` now starts the Loopflow prompt with a blank row, so each
block's header sits one row below the previous block's output. Focused tests:
`swift test --filter "GhosttyShellBlockTests|LocalWaveAgentLauncherTests"`,
13 passed. Loopflow Dev was relaunched with this build; Jack has not yet said
whether the spacing is now right.

Known shape of this fix (interpretation, not Jack's): the blank row belongs to
the block below it, under that block's hairline. A failed block is therefore
red one row above its header and flush at its bottom edge. Splitting the gap
evenly needs the overlay to draw block edges at half-row offsets, which is only
safe when Loopflow owns the prompt. A customized prompt gets no added row.

## Warp block affordances

Jack named two:

1. **The bar on the right.** Not yet pinned down: this may be the per-block
   action cluster Warp shows at a block's top-right on hover, or a marker along
   the right edge. Ask Jack which before designing it.
2. **A header holding what was in the prompt when the command was sent.** Each
   block keeps the directory, branch and other context as they were at
   submission, not as they are now. Loopflow's header is printed prompt text in
   the terminal rows, so it already is that snapshot, for directory and branch
   only.

The rest is recalled from Warp's product, not observed in Jack's installed
version during this demo; verify before building against it:

| Area | Warp behavior |
| --- | --- |
| Structure | Every command and its output is one block with its own padding and a divider; the input editor is separate, pinned at the bottom |
| Header | Prompt context captured at submission (directory, git branch and dirty state, and whatever else the prompt carried); a long block's header sticks to the top while its output scrolls |
| Status | Failed block gets a red background and red edge; running block is distinguished; duration is shown for longer commands |
| Selection | Click selects a block; shift-click a range, command-click to add; arrow keys move between blocks; text selection inside a block replaces block selection |
| Copy | Copy command, copy output, or both, from the block menu and by shortcut |
| Hover actions (top-right) | Bookmark, filter the block's output, context menu, AI on the block |
| Context menu | Copy variants, share (permalink), find within block, reuse the command in the input, save as workflow, scroll to block top/bottom |
| Navigation | Jump to previous/next block and between bookmarks; scroll to the start of the selected block |
| Other | Notification when a long command finishes; failed blocks offer an AI explanation |

Loopflow today: one-click block selection, whole-block copy, hover tint,
failure tint from exit status, directory/branch header, hairline divider and
left accent bar. Everything else in the table is absent. The design already
defers per-part copy, multi-selection, bookmarks, sharing and
exit-code/duration badges until the core interaction is accepted.

## Jack's Warp reference screenshot

Jack supplied `~/Desktop/Screenshot 2026-10-05 at 8.02.17 PM.png` (Warp, `ls`
then a failing `kill -9`). Observed in it:

- **Header line**: smaller and dimmer than everything else, grey. Holds
  directory, `git:(branch)`, file and line counts, PR number and the duration
  in parentheses, all as they were at submission.
- **Command line**: bold and brighter than output, at the body size.
- **Output**: regular weight, with a gap of roughly half a row below the
  command.
- **Padding**: about half a row inside each block above the header and below
  the last output row, so adjacent blocks are separated by about a full row
  with a hairline in the middle.
- **Failure**: the whole block, padding included, has a dark red background,
  and a brighter red bar runs down its **left** edge. The successful `ls` block
  has no bar and no fill.
- **Live prompt**: separate from the blocks; context shown as outlined chips.

No right-side bar is visible in this screenshot. Jack wrote "the bar on the
right"; the only bar shown is the failed block's left edge. Unresolved which he
meant.

Differences from the branch as demoed: Loopflow's header is at body size and
carries the branch; the command is not bold; there is no gap between command
and output; the blank row sits wholly above the header, not split around the
hairline; every block has a left accent bar, not only failed or selected ones.

## What Jack wants from Warp's blocks

Jack's list: the directory in the header, the bar on the right, the spacing,
a header set in a different size from the command and output, and a header in
different colors. Git and PR context are not needed "per se".

Proposals that follow (mine, not agreed):

- **Draw the header in the overlay, not in terminal rows.** A terminal grid has
  one font size, so a smaller or larger header cannot be prompt text. The design
  rejected overlay headers because they need per-block directory storage and a
  reserved row; Jack's font-size request reopens that. The `lf2` patch would
  record the working directory on the prompt row at submission (the shell
  already reports it through OSC 7), the Loopflow prompt would keep a blank
  header row for the overlay to draw into, and the header could then also
  carry spacing that is not a whole row.
- **Drop the branch from the header**, leaving the directory. Not done yet:
  Jack said it is not needed, not that it must go.
- **The bar**: draw the left accent only on failed and selected blocks, as in
  the screenshot, unless Jack means something else on the right.
- **Half-row padding**: with the header drawn by the overlay, shift each block's
  fill and hairline by half a row so the blank row is shared between the block
  above and the block below, and a failed block is red evenly top and bottom.
- **Bold command**: the Loopflow prompt can end with `%B` and reset in `preexec`;
  zsh's `zle_highlight` default region does the same without touching output.
  Half-row space under the command is not available in a cell grid; skip it.

Proof for that pass: the headless block-layout and zsh-hook tests extended for
the stored directory and the shifted frames, the patch's Zig tests for the
directory surviving reflow, and Jack comparing a Desktop shell with this
screenshot.

## Unresolved

- Whether one blank row is the right amount between blocks, and whether the
  red block's uneven top/bottom reads as wrong.
- What "the bar on the right" is, and whether Jack wants it in this Task.
- Which Warp affordances Jack wants next. He asked for the list, not for any
  of them to be built.
- The step-by-step observations listed above as unconfirmed.
- `lf2` publication (`questions.md` 1): no other checkout can build this branch
  until the artifact is at `bin.loopflow.studio`.

## Recommended next action

Jack looks at the relaunched Loopflow Dev: `ls`, `sdl`, a third command, and
confirms or corrects the gap. Then decide the right-side bar and any Warp
affordance to pull into this Task versus a follow-up. Proof for the spacing is
Jack's eye on a real shell; proof for anything interactive added later is the
display suite (`LOOPFLOW_NATIVE_TESTS=1 swift test --filter "Embedded terminal"`),
which has still not run for `lf2`.

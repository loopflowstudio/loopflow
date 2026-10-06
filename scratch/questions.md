# Open questions and assumptions (LOO-381)

Recorded at kickoff, 2026-10-05. None of these has Jack Heart's confirmation.

1. **Publishing the `lf2` GhosttyKit artifact.** Failure tint and unified
   selection need a patch change, so `Package.swift` must pin a new artifact at
   `bin.loopflow.studio`. Publication has been a separately authorized action.
   Assumed: uploading a new versioned file that `main` does not reference is
   additive and may proceed for the demo build; if credentials or authorization
   are missing, slices 3–4 are verified against the local framework without
   committing a local path, and the demo waits on publication.
2. **Loopflow replaces the default zsh prompt in Desktop shells.** Assumed
   acceptable because Jack's screenshots show the stock `user@host` prompt and
   name its repetition as a problem. Customized prompts are untouched. Header
   contents were directory and branch at kickoff; see 7.
3. **Left padding applies to provider panes too** (12pt), since "text tight to
   the left edge" reads as a terminal-wide complaint. Block decorations remain
   shell-only.
4. **Stripping `PAGER`, `GIT_PAGER`, `GH_PAGER` and `CI` at GUI launch.** These
   are agent-shell settings in the observed leak; a person who exports them in
   their rc files gets them back in shell panes, but not in a provider launched
   before the login shell starts.
5. **Clicking a block while typing.** Settled in the polish pass: the first
   key that goes to the shell ends the block selection. See 11.
6. **Jack's screenshots were not available to kickoff.** The plan rests on the
   Task's description of them. The color cause matches the app instance running
   at the screenshot times, but the screenshots themselves were not compared.

Added 2026-10-05 after Jack's demo feedback. Not confirmed by Jack.

7. **The branch left the header.** Jack said git and PR context are not needed
   "per se"; the header now holds the directory only, which also removes a
   `git` call per prompt. Restoring it is one prompt segment.
8. **Header size and color.** 11pt system monospace in the dim grey `#A39B93`
   against the 13pt body. Jack asked for a different size and color, not these values.
9. **"The bar on the right" is not built.** His screenshot shows only a left
   bar on the failed block. The left accent still appears on hover as well as
   on failed and selected blocks.
10. **The command is bold** through zsh's `zle_highlight`, under the Loopflow
    prompt only. Jack's screenshot shows it; he did not name it.

Added 2026-10-05 in the polish pass. Not confirmed by Jack.

11. **Escape ends a block selection and still reaches the shell.** Swallowing it
    would make a selected block hold the keyboard for one key; in zsh vi mode
    the Escape also changes mode. Ghostty treats a text selection the same way.
12. **Option-letter is not Alt in Desktop.** Standalone Ghostty on a US layout
    sends `ESC b` for Option-B; the view inserts the composed character. Fixing
    it means translating modifiers per layout before text input, which needs a
    real pane to check. Not in the requested key list; left for Jack.
13. **Selected failed blocks are only slightly redder than unselected ones**, to
    keep dim text at 3:1. A deeper red is the alternative.

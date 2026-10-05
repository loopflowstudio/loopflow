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
   contents are directory and branch only.
3. **Left padding applies to provider panes too** (12pt), since "text tight to
   the left edge" reads as a terminal-wide complaint. Block decorations remain
   shell-only.
4. **Stripping `PAGER`, `GIT_PAGER`, `GH_PAGER` and `CI` at GUI launch.** These
   are agent-shell settings in the observed leak; a person who exports them in
   their rc files gets them back in shell panes, but not in a provider launched
   before the login shell starts.
5. **Clicking a block while typing.** A block selection persists until another
   click or selection; typing does not clear it. Warp's behavior here was not
   checked.
6. **Jack's screenshots were not available to kickoff.** The plan rests on the
   Task's description of them. The color cause matches the app instance running
   at the screenshot times, but the screenshots themselves were not compared.

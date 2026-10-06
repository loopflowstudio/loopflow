# v0.13.5

v0.13.5 keeps saved conversations retryable and prevents a Task's own remote commits from blocking delivery after a sync. Session identity now controls conversation actions while captured history stays intact. Desktop terminals also gain more consistent colors and command blocks that are easier to select and copy.

## Resume the conversation without losing its history

Conversation control now follows the durable Session across provider replacement. Recovery preserves the saved thread, review feedback, and historical outcomes while rejecting writes from replaced providers.

- Session actions accept durable Session or native conversation IDs; capture keys remain available for inspection and replay.
- Failures before a provider spawn request leave saved Sessions retryable, and saved provider threads load before account selection.
- On macOS and Linux, an observed restart of the same host can permit replacement of a stranded engine when the recorded driver is unchanged. This requires evidence recorded before the restart; a missing PID or failed Exec alone is insufficient.
- Recovery preserves unknown outcomes and existing Flow state.

## Keep delivered work eligible to finish

Syncing a Task's own remote branch no longer moves its recorded PR base to that branch's tip. Operators can also explicitly accept eligible historical Exec uncertainty when completing delivered work, without rewriting the uncertain history.

- Syncs against non-default branches measure the Task PR's range against its real upstream, preserving its own commits for later sync and delivery.
- For an already affected Task, the next sync or land repairs the stored base when the remote-tracking reflog proves it was the PR's own tip. Repair does not rewrite history; unrelated ancestry or missing evidence still prevents it.
- `lf task complete TASK_ID --accept-unknown-exec EXEC_ID --summary 'Accepted historical uncertainty; delivery verified'` records explicit acceptance in Task history. Repeat the flag for each eligible Exec.
- Acceptance retains the checkout and the unknown Exec outcome. Current execution protections, PR settlement, and other completion requirements still apply; acceptance grants no process control, cleanup permission, or authority to start more automated work.

## Read and copy Desktop commands more easily

Desktop terminal settings are isolated from inherited launcher settings, keeping CLI colors consistent while preserving provider credentials. Command and text selections now replace each other, and copying uses the current selection.

- Default macOS zsh prompts gain a directory header, bold command input, and spacing between blocks. Custom prompts remain unchanged, and provider panes retain their native TUI behavior.
- Red backgrounds mark reported command failures; selections use cream, with readable dim text.
- Selected blocks survive resizing, reflow, scrolling, and right-click. Typing or Escape clears them.
- Command-Up and Command-Down navigate between prompts.

## Operational notes

Integrations should use Session identity for conversation control. Runtime context replaces `LF_RUN_ID` and `LF_RUN_DIR` with `LF_CAPTURE_KEY` and Session/Exec provenance; Wave JSON exposes `history` instead of `runs`. Existing `runs/` capture directories, historical keys, and payloads remain in place without relocation or migration.

Release installation checks now require Docker. Candidate preflight runs in disposable Ubuntu 24.04 ARM64 containers with networking disabled; public installer checks run without host mounts or forwarded credentials. Verification compares the selected CLI's manifest digest and actual bytes with the public artifact, then exercises the installed launcher. Missing Docker, rejected preflight, or failed cleanup prevents success. Separate native macOS version/help checks remain.

Recorded checks include passing Task PR range regressions and 42 release publisher tests. Real container execution, a real host restart, the full installed Task-completion writeback path, and display-dependent terminal interaction remain unverified in the supplied evidence. Session preservation was checked with simulated providers; final affected-suite verification was still open.

## Small changes

- Desktop performance tooling now measures first launches from fresh bundle paths separately from repeat launches and records executable size. A benchmark-only stripping option supports comparison; release packaging is unchanged. The recorded measurements approximate an update launch, not a notarized installation, and do not establish the 400 ms first-frame target.

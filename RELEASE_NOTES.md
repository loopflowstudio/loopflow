# v0.13.1

v0.13.1 makes conversations easier to continue and delivery failures easier to recover. Claude and Codex conversations use their native homes by default, Desktop keeps unfinished interactive work visible, and New Session can capture ideas as Tasks without disturbing existing work. Scheduled releases now retain the evidence needed to distinguish verified publication from deferred or failed attempts.

## Continue conversations across Loopflow and native CLIs

Conversations no longer need separate account homes by default. Loopflow shares Claude and Codex history with their ordinary CLIs while retaining explicit isolation and the original home mode when resuming existing work.

- Switch the shared login with `lf account <provider> use <email>`, preserving refreshed and previously unknown credentials.
- Choose `--isolate` or `--shared`, or set the `isolate` configuration default. Existing account-pinned conversations remain isolated.
- Use native conversation IDs in `lf session` commands; connecting imports previously untracked conversations.
- Close owned Codex engines when their driver exits while preserving saved history, live takeovers, and unrelated conversations.

## Find unfinished work and capture the next idea

Ordinary Desktop and CLI Session lists now focus on unfinished interactive conversations and current authored reviews. New Session adds a repository-local skill picker so exploring an idea can lead to Tasks while existing work stays in place.

- Choose a skill beside New Session. The default is `capture-tasks`, and the choice is remembered per repository; selecting a skill does not launch it.
- Launch from the repository sidebar or a Wave context menu. A selected Task supplies its parent Wave as context while Task selection and existing terminal layouts are preserved.
- Use `capture-tasks` to explore ownership across Waves and repositories and file self-contained Tasks without starting workers or binding the capture conversation. The guidance preserves successful filings and reconciles uncertain writes before retrying.
- Keep navigation counts aligned with the filtered Session list. Filtering preserves selection, prepared commands, drafts, and retained native surfaces; a finished turn does not explicitly complete a Session.
- Enable Show headless Sessions or use `lf session list --interactive all --history` for full inspection. `--needs-me` narrows the selected mode.

## Recover launches and delivery without losing evidence

This patch addresses failures where one stalled operation could block unrelated work or hide a retained delivery. Release accounting now follows each scheduled opportunity through execution, verification, and its product outcome.

- Prevent stalled native dispatch from holding SQLite across provider I/O. Session-scoped locking retains stale-driver rejection while allowing unrelated database writes.
- Confirm Linear creation, edits, and completion against the exact issue and its Project association, avoiding a full-Wave refresh as a prerequisite for local preparation and retaining duplicate-safe retries.
- Preserve pending PR deliveries when retiring obsolete Home supervision, including delivery intent, failures, CI state, and terminal history.
- Start detached workers in their requested checkout even when the tmux server was left in a deleted directory. Spawn errors identify the requested directory.
- Inspect verified publication, verified no-change, deferred work, and failures with `lf release history`; record repair ownership through `lf cron disposition`.
- Catch up missed release times with one execution per wake, retaining original timing and unfinished candidates. Multiple missed opportunities covered by one execution do not become multiple successful releases.
- Recover interrupted release checks using process and lock evidence. Publication settlement requires the exact candidate, public artifact read-back, and installer smoke checks; retained artifacts support retries of missing publication stages.

## Operational notes

Shared account switching also changes the login used by plain Claude and Codex. Running Codex engines retain their original login until restarted; an in-flight turn may fail once, with automatic recovery for headless runs. Shared Codex switching requires file-based credential storage. Forwarded Codex login uses an experimental protocol and requires Loopflow’s engine.

Release locks and checkout leases now extend through Git, PR, hook, CI-repair, and publisher children, preventing controller exit from permitting overlapping publication or premature checkout removal. Schedule replacements on the same Home retain unfinished candidates; changing Homes leaves unresolved work available for repair without transferring execution authority.

Upgrade running binaries to receive the dispatch fix. Desktop’s new skill launch requires a CLI with `capture-tasks` and explicit skill launch support. Automated regressions cover the release recovery paths, but installed acceptance, public artifact and installer proof, and repeated automatic release execution remain follow-up validation. Native picker interaction and live cross-repository Task capture also remain unverified end to end.

## Small changes

- Passing Dependabot updates now enter the required merge queue through a dedicated repository token, with retries after successful CI and required checks preserved.
- Update Ruff, thiserror, tiktoken-rs, tokio-tungstenite, and dirs.
- Generated PR reviews focus on behavior, data models, and APIs.

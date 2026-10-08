# Session titles (LOO-439)

Jack Heart requested implementation through demo on October 8, 2026, without
provider accounts or changes to his running cmux windows. No landing.
Reconciled October 8 against the source and recorded compression checks.
Request-derived lf naming is implemented; plain-native naming remains unresolved.
The earlier feedback about random seeds predates `91c4d07f9`. Its plain-native
requirement still applies. No Task acceptance has been waived.

## Design and implementation

The Session name in SQLite owns lf presentation. New names use a three-word
excerpt of the attributed request, excluding common articles and terminal controls,
within the existing 80-character limit. Read UserMessage assets in either prompt
channel: LOO-429 moved the request into the system file. The generic task-channel
trigger and injected instructions are not naming inputs. Unassembled library
calls use their separate task prompt. This is an excerpt, not a semantic summary;
agent suggestions can improve it without replacing human names.

Without request text, a concrete skill supplies the purpose. Generic session and
operator skills use the Task title or workspace name. New Task primary Sessions
use the Task title. Existing names, human precedence, native identity and history
are preserved; this does not retroactively repair old generated names.

Bound assembly attributes the caller's request separately from its inherited Goal,
preserving context bytes. Its CLI fixture exposed the lost request attribution.

Terminal presentation prefixes the Task identifier and limits its purpose to three
words. Taskless presentation uses the stored Session name. Native launch/reconnect
and library `run_agent` share one title module. Initialize OSC 0 before provider
spawn; live cmux updates use its public workspace/tab commands and only identifiers
in the launching environment. The owning attachment observes names through a
read-only store and stops on driver replacement. Host commands are bounded; failure
disables updates until reconnect. Other terminals pick up renames on reconnect.

Claude receives its native name option; lf disables Claude/Codex terminal-title
updates only for its titled interactive launch. Native stdio and host message hooks
are preserved. No live OSC writer, transcript rewriting, model call, provider
wrapper, PTY relay, terminal-address registry or schema change was added.

Random word pairs and duplicate wait loops are removed. Both interactive paths
share `TerminalTitle`; `engine/process.rs` owns the bounded wait. Request extraction
and title selection are separate functions, without another naming owner.

## Headless evidence

The public CLI fixture starts from `Plan store migration for archived tasks`:
Task-bound Claude/Codex automatically show `INF-123 Plan store migration`, follow
`Release notes`, then reconnect with that human name and native resume arguments.
Its twelve account-free launches cover Taskless, outside-cmux, failed-host and
timed-out-host paths in isolated Homes with external-network denial. Only stderr
has a PTY; pipes carry stdin/stdout. This proves no native TUI, hooks, shims or
rendered agreement. Lifecycle/capture/attribution fixtures cover Task-primary names,
human precedence, library/headless naming and source/steer provenance.

Gate retains driver-replacement and missing-cmux-executable checks, and Flow-bound
launch acceptance. The outside-cmux test omits host identifiers; it does not prove
missing executable behavior. Task-bound execution does not prove a captured Flow.

## Plain native mechanism: unresolved implementation

Repository guidance and lf's title transport do not control plain native programs.
The earlier documentation research identified different provider mechanisms:

- [Claude hooks](https://code.claude.com/docs/en/hooks#sessionstart) support
  `sessionTitle` on startup/resume, and UserPromptSubmit receives the request.
  That provides a native integration direction, not a tested implementation here.
- [Codex hooks](https://learn.chatgpt.com/docs/hooks) document context output for
  SessionStart/UserPromptSubmit, with no title-setting result.
  [Codex app-server](https://learn.chatgpt.com/docs/app-server) supports
  `thread/name/set` for loaded or persisted threads. This does not establish how
  a plain running TUI adopts a name written through a separate server, or how its
  terminal title and cmux hooks agree.

A native integration design and implementation remain substantial work: how hooks
reach plain starts across Loopflow repos, when the first request supplies the name,
how resume preserves human names, and how Codex's running TUI adopts it. The design
choice remains open between a supported native hook/title path and a supported
provider naming API; the latter still needs evidence of live TUI adoption. Neither
path is accepted or proved. Writing native storage or adding a competing terminal
writer is not an accepted substitute. No hooks or provider settings were installed.
Repository guidance alone does not meet the universal plain-start/resume requirement.

LOO-429's system-file path is integrated and the fixture excludes its generic
trigger from naming. That proves lf attribution, not that moving context fixes
native title generation. LOO-428's configured shims remain part of acceptance;
LOO-422 owns host status, and no second status transport is introduced here.

## Demo and review boundary

cmux denied the earlier creation attempt outside its process origin; no windows or
accounts were used. Demo needs an allowed cmux-origin process, a separate unfocused
workspace, candidate bytes and synthetic providers. Sidebar/tab/window agreement,
latest-message line, herdr and LOO-428's shim path remain unobserved. No acceptance,
publication or landing is claimed.

Live rename changes cmux workspace/tab names only. OSC 0 and Claude's native name
are set at launch; Codex receives terminal-title suppression, not a native thread
rename. Agreement among sidebar, tab and window bar after rename is therefore an
open acceptance condition. If the host does not propagate its control-channel
rename to the window bar, implementation remains; demo cannot waive it. Native
stdio must stay unchanged, with no concurrent OSC writer.

Review: repaired request/Goal attribution and Task-primary titles. Display reads
grant no authority. A shared workspace follows its latest launch/rename; each
surface retains its own name. The request excerpt is deterministic, not a semantic
summary; natural requests with generic opening words still need naming-quality
judgment. Release's child memory reinforces the boundary here: a shared helper's
proof does not establish each entry point. Gate retains captured-Flow acceptance;
plain-native and configured-shim behavior need their own evidence.

Checks: realign `git diff --check` passed; prose-only reconciliation reuses compression's build, 4 naming/capture tests, 12-launch CLI fixture, fmt and Clippy passes; attribution/lifecycle results: `91c4d07f9:scratch/every-lf-session-in-cmux.md`; automated acceptance remains gate-owned, visible agreement demo-owned.

Earlier transport/demo evidence and the full prior plan:
`04027f096:scratch/every-lf-session-in-cmux.md`. Public
[cmux naming documentation](https://github.com/manaflow-ai/cmux/blob/main/docs/workspace-auto-naming.md)
and help were studied; no cmux/herdr source, tests or configuration were read or reused.

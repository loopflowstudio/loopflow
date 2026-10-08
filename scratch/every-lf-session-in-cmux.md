# Session titles (LOO-439)

Jack Heart requested implementation through demo on October 8, 2026, without
provider accounts or changes to his running cmux windows. No landing.
Reconciled October 8 after the Task-bound launch exposed missing request attribution.
The lf naming implementation is present; plain-native naming remains unresolved.

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

Bound prompt assembly now attributes the caller's request separately from its
inherited Goal. The first Task-bound CLI test otherwise showed the Task's title
instead of the supplied request. Context bytes and provider input stay unchanged.

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

## Delete — do not maintain

- Removed `engine::naming::word_pair`, both word lists and their exclusive test.
- Replaced random-name assertions with requested-work assertions, preserving
  lifecycle, name precedence, history and reconnect coverage.
- The earlier separate child wait loops and unused launch environment clone were
  already removed; `engine/process.rs` owns the common bounded wait.

## Headless evidence

The account-free public CLI fixture now launches from an actual request, without
seeding or manually assigning a name. Claude and Codex in a bound Task show
`INF-123 Plan store migration` for `Plan store migration for archived tasks`.
The fake host records the automatic workspace/tab names before any rename, then
follows `Release notes`; reconnect retains that human name and native resume args.
Taskless, outside-cmux, failing-host and timed-out-host paths remain covered.
Twelve launches use synthetic providers, isolated Homes and external-network denial.
Only stderr has a PTY; stdin/stdout are pipes. This proves neither full native TUI
behavior nor cmux hooks, shims or rendered agreement.

The Task-primary lifecycle fixture proves a new primary uses its Task title;
existing lifecycle coverage proves suggestions cannot replace human names.
Focused capture coverage proves requested-work naming on library/headless launches.
Existing context-attribution tests retain source and steer provenance.

Gate retains driver-replacement and missing-cmux-executable checks, and Flow-bound
launch acceptance. The outside-cmux test omits host identifiers; it does not prove
missing executable behavior. Task-bound execution does not prove a captured Flow.

## Plain native mechanism: unresolved implementation

Repository guidance and lf's title transport do not control plain native programs.
Current official documentation establishes different provider mechanisms:

- [Claude hooks](https://code.claude.com/docs/en/hooks#sessionstart) support
  `sessionTitle` on startup/resume, and UserPromptSubmit receives the request.
  That provides a native integration direction, not a tested implementation here.
- [Codex hooks](https://learn.chatgpt.com/docs/hooks) document context output for
  SessionStart/UserPromptSubmit, with no title-setting result.
  [Codex app-server](https://learn.chatgpt.com/docs/app-server) supports
  `thread/name/set` for loaded or persisted threads. This does not establish how
  a plain running TUI adopts a name written through a separate server, or how its
  terminal title and cmux hooks agree.

A native integration design must settle hook distribution across Loopflow repos,
human-name precedence and the Codex TUI update path. Writing native storage or
adding a competing terminal writer is not an accepted substitute. No hooks or
provider settings were installed. The universal plain-start/resume requirement
remains unmet; it is not reduced to a screenshot or waived by these source tests.

## Demo and review boundary

The earlier real cmux creation attempt was denied before creating a workspace,
because the caller was outside cmux. No existing windows or provider accounts were
used. Next visible evidence needs an allowed cmux-origin process in a separate
unfocused workspace, the candidate bytes and synthetic providers. Sidebar/tab/window
agreement, latest-message line, herdr and LOO-428's configured shim path remain
unobserved. No human acceptance, publication or landing is claimed.

Review findings: the real Task entry point exposed request/Goal attribution loss,
now repaired. Primary Sessions also needed the Task title rather than only its ID.
No display read acquires provider authority. Multiple Sessions in one workspace
still leave its name following the latest launch/rename; demo judgment remains.

Checks: `cargo test -p loopflow --lib --bin lf --test session_cli_tests --test session_lifecycle_tests --no-run`, focused binaries under `uv run python scripts/test_network.py` (Task-bound title fixture: 12 launches; capture/naming: 2; attribution: 3; Task-primary/human-name lifecycle: 2), `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `git diff --check` passed; remaining automated acceptance belongs to gate, visible agreement to demo.

Earlier transport/demo evidence and the full prior plan:
`04027f096:scratch/every-lf-session-in-cmux.md`. Public
[cmux naming documentation](https://github.com/manaflow-ai/cmux/blob/main/docs/workspace-auto-naming.md)
and help were studied; no cmux/herdr source, tests or configuration were read or reused.

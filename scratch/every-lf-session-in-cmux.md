# Session titles (LOO-439)

Jack Heart requested implementation through demo on October 8, 2026, without
provider accounts or changes to his running cmux windows. No landing.
Reconciled October 8 against the implementation and Release's entry-point lesson.
The mechanism below is the implementation choice; visible acceptance is pending.

## Design

The existing Session name remains authoritative. Terminal presentation prefixes
a Task's identifier and keeps the purpose to three words; taskless Sessions use
their recorded name. No generated summaries of injected instructions. Reconnect
reads the current name, and cmux follows live `lf session rename` updates from
the Session's own attached process, never from the renaming command's terminal.

Initialize OSC 0 before spawning the native provider. Do not add a second writer
to the provider's live terminal stream: LOO-398 already demonstrated partial OSC
writes. Native stdio, provider hooks and status remain intact. cmux's explicit
workspace and surface names need its public CLI; update only the identifiers
in the launching terminal's environment, with bounded commands. Outside cmux,
renaming takes effect on reconnect. No terminal-address registry or schema.

Claude receives its Session name through its native name option. Provider title
updates are disabled only for lf's titled interactive launch. Plain native starts
remain provider-owned; repository guidance tells them to name the actual work.
Prompt guidance cannot guarantee a provider's automatic title or repair an old
native conversation. That acceptance is unresolved, not a claimed fix.

## Implemented shape

- The human-present surface now requests short, work-specific names; SQLite
  retains human-name precedence. Repository guidance also covers native starts.
- One title module serves both interactive launch paths. There was no earlier
  terminal-title emitter or cmux integration to replace.
- Native launch and cmux commands share the existing child wait/timeout loop in
  `engine/process.rs`; separate polling loops and the unused launch environment
  clone are removed. The title formatter is private to its module.
- LOO-429's system-file context path is integrated through #1498. Its source
  presence does not establish native automatic naming after that change.
- There is no transcript rewriting, summarizer, provider wrapper, PTY relay,
  terminal-address registry or schema change.

## Evidence and remaining work

Installed cmux is 0.65.0 (108), dda24fbd2. Public help and documentation were
studied; no cmux/herdr implementation, tests or configuration were read or reused.
`cmux new-workspace --cwd /tmp --focus false --command ...` refused access because
this process was not launched inside cmux; no workspace was created.

The shared title module is used by native Session launch/reconnect and library
`run_agent`. It observes the Session through a read-only store connection and
stops cmux updates if the driver changes. The first public-CLI fixture exposed
the second launcher; connecting it fixed the missing name. The fixture also had
to close its own retained PTY descriptor and seed native identity to exercise
reconnect instead of orphan recovery. No production recovery rules changed.

The account-free fixture passes for Claude and Codex launch, live rename and
reconnect, plus outside-cmux, failing-host and timed-out-host cases. Only stderr
uses a PTY; stdin and stdout are pipes. The fake host records workspace/tab names;
the fixture verifies provider output and a single pre-spawn OSC. It proves neither
native full-terminal behavior nor cmux hooks or rendered UI. Task prefixing has
unit coverage; the public fixture creates a taskless Session.

Release's entry-point lesson applies here: a formatter proof cannot establish
Task/Flow launch behavior. Remaining headless acceptance covers a bound Task
through its real launch path, driver replacement stopping host updates, and a
missing cmux executable. The existing outside-cmux case omits host identifiers;
it does not exercise executable discovery failure. Host update errors currently
disable further updates for that attachment; reconnect retries initialization.

Name quality remains separate from title transport. `session_record.rs` seeds
names from the invoked skill or a word pair; this change does not replace that
selection. Agent suggestions can improve generated names, but an operator skill
name or old instruction-derived name can still be displayed unchanged. Automatic
work-specific names, including plain native starts and resumes, remain required;
guidance alone does not meet the Task's universal naming requirement. A reliable
mechanism beyond the current projection remains implementation work, not merely
a missing screenshot.

## Demo boundary

Visible sidebar/tab/window agreement, the latest-message line and herdr behavior
remain unobserved. The real cmux command was denied before workspace creation.
Next proof needs an allowed cmux-origin process creating a separate unfocused
workspace, with these source bytes and synthetic providers. Native plain/resumed
automatic naming remains an implementation/acceptance gap; repository guidance
is not a deterministic fix. No provider accounts or existing windows were used.
No human acceptance, publication or landing is claimed.
The configured provider-shim path (LOO-428) still needs that same live check;
the synthetic provider fixture does not exercise cmux's shims or hooks.

Review finding: title presentation must not introduce a competing native-output
writer or terminal-address registry. The implementation keeps native stdio,
uses SQLite names, and bounds host commands; ordinary terminal renames wait for
reconnect. A workspace with multiple Sessions follows the most recent launch or
rename; that workspace-level choice remains for demo judgment.

Checks: realign `git diff --check` passed; `lf context --skill realign --json` fits budgets. Reused compression passes (Rust unchanged): `cargo test -p loopflow --lib engine::terminal_title::tests` (2), `cargo test -p loopflow --test session_cli_tests terminal_titles_follow_session_rename_and_reconnect_without_provider_accounts` (10 launches), `cargo test -p loopflow --test agent_tests launch_batch_times_out` (cleared environment, source `LF_BIN`), `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`; remaining headless acceptance belongs to gate, visible/native judgment to demo.

References studied: [cmux naming documentation](https://github.com/manaflow-ai/cmux/blob/main/docs/workspace-auto-naming.md),
[Claude CLI](https://code.claude.com/docs/en/cli-reference),
[Codex config](https://developers.openai.com/codex/config-reference/).

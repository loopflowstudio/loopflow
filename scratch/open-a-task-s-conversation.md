# Open a Task's conversation

Jack Heart requested autonomous repair of lf-8fed729 on October 12, 2026.
The supplied directive requires ensure → connect, replace → connect, recovery
of stranded interactive providers, and no duplicate live provider. No prior
design was supplied; this is the implementation design.

Prepare the primary Session and input in `ensure`/`replace`; `connect` owns
the foreground launch. Keep scope/launch locks, Task selection, native history,
workspace placement, and replacement transaction. No schema change.

October 12 read-only inspection confirms both reported Sessions lack native
history and `replace` retired the first. Explicit opening must retry a retired
Session too, preserving its completion marker and the successor's primary
selection. Replace input against the observed completion marker (reject races),
and permit publication of that exact capture. This matches existing explicit
opening of retired Sessions with native history; no reopening lifecycle is added.

An interactive provider with an exact recorded PID/birth observed dead may
settle under the existing attachment fence. A live or unknown interactive
provider cannot authorize resume, signaling, or another launch. Headless
process-group death rules remain unchanged. Death does not invent success.

## Delete — do not maintain

- Primary `start` background launch: replace with input preparation.
- `serve-conversation` hidden command and its sole serving function; the unused
  `start_home_session` wrapper.
- tmux conversation liveness/start helpers, launcher name, mock launcher sets,
  and exclusive launcher/retry tests. Process evidence owns liveness; preserve
  selection, workspace, replacement, input and native-history coverage.
- Completed-Session refusal in explicit opening/explanation and input replacement;
  retain completion evidence and reject changes since the input snapshot.

## Review findings

Preparation now re-reads the Session after acquiring its launch lock so an
opener that won the lock cannot leave `ensure` publishing a stale input.
The retired/no-history case found in the actual reported records is covered
without clearing completion or stealing primary selection. Unknown native
identity stays unknown; no generic process reaper or alternate launch path was added.

## Remaining

- Gate: affected Session/connection/store suites and automated acceptance;
  installed recovery of the two reported Sessions and real Codex terminal
  interaction remain unproved. Stand-in CLI tests establish source behavior.
- Publication and landing remain with the supplied Flow's later steps.

Checks: `cargo test -p loopflow --lib` with primary/native_resume/retired_input filters and `cargo test -p loopflow --test session_lifecycle_tests ensured_` — 10 passed; `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `git diff --check` pass; broader suites and installed acceptance deferred to gate/CI.

# Session titles (LOO-439)

Jack Heart requested implementation through demo on October 8, 2026, without
provider accounts or changes to existing cmux windows. No landing. Request-derived
lf naming and native hooks are implemented. The universal native and host agreement
requirements remain unfinished; the gaps below are not waived.

## Implementation and ownership

The Session name in SQLite owns lf presentation. Names use a three-word excerpt of
the attributed request, excluding common articles and terminal controls, within
80 characters. UserMessage assets can come from either channel; LOO-429's generic
trigger and inherited Goal are not naming inputs. Concrete skills, Task titles
and workspace names supply missing purpose. Task-primary Sessions use Task titles.
Existing names and human precedence survive. This is an excerpt, not a semantic
summary. Generic opening words remain a naming-quality limitation.

Terminal presentation prefixes the Task identifier and limits its purpose to three
words. Taskless presentation uses the stored name. `TerminalTitle` serves both
interactive entry points. OSC 0 is written before spawn; live cmux updates use
workspace/tab control commands with launching-environment identifiers. Read-only
name observation stops on driver replacement; bounded host-command failure disables
updates until reconnect. Other terminals update on reconnect. Claude receives
`--name`; titled lf launches suppress Claude/Codex terminal-title updates. Native
stdio and host message hooks are retained.

Plain native hooks share the same request-excerpt function. Claude returns its
supported `sessionTitle`, preserving `session_title`; an unnamed resume reads its
first non-meta user request. Codex uses `thread/read` and `thread/name/set` on the
existing shared app-server's WebSocket control socket, preserving existing names.
No second engine, provider-history writer, model call or concurrent OSC writer is
introduced. Native providers own terminal output. Hooks skip conversations already
recorded as lf Sessions, so later native turns cannot create a competing title.

Published installation adds UserPromptSubmit and resume SessionStart hooks under
the standard `.claude` and `.codex` homes, preserving other hooks, settings,
permissions and symlink destinations. Hook callbacks act beneath `.lf` directories.
Codex's own hook trust review remains in force. No native settings were installed
on Jack's machine. Installation-entry acceptance remains gate-owned; helper tests
prove merging and repeated installation, not a published installation.

## Native evidence and counterexamples — October 8

Claude Code 2.1.295 and Codex 0.161.0 ran real TUIs in private PTYs with fresh homes,
no real credentials and external-network denial. A loopback fake API returns one
fixed response; it proves transport and naming, not model judgment. First requests
produce `Plan store migration` in native title output. Native `/rename`, another
request and native resume preserve `Manual release notes`. Claude also names a
previously unnamed resume from its original request.

`tests/e2e/native_titles.py` retains the reproducible probes. Initial and resumed
OSC captures are written to the caller's output directory. These are native TUI
proofs, not rendered cmux/herdr or configured-shim proofs. The first exploratory
probe separately established that an external API rename reaches the active Codex
TUI's OSC output and footer; the later hook probes establish actual first-request
naming rather than mere API acknowledgement.

Two Codex counterexamples invalidate a universal hook-only design:

- `--dangerously-bypass-hook-trust` selected embedded execution in the isolated
  probe, with no shared control socket. The normal shared-server API therefore
  cannot name that TUI. `--no-daemon` has the same missing-endpoint limitation.
  No competing writer or storage edit substitutes for live adoption. LOO-428's
  configured shim path still needs its own proof and supported naming mechanism.
- Reconnecting to an already-loaded unnamed thread did not invoke SessionStart.
  The callback can derive and apply its original request through the API when
  explicitly invoked, but a native reconnect did not invoke it. The
  `--unnamed-resume` Codex probe retains this failing acceptance case. A native
  attach event/API or another supported integration is still needed. Existing
  named resumes pass; that does not cover this counterexample.

The current provider integration must not be called universal or accepted while
those execution shapes remain uncovered. Changing a provider/host configuration
or replacing native stdio is not authorized by these probes.

Sources: [Claude hooks](https://code.claude.com/docs/en/hooks),
[Codex hooks](https://learn.chatgpt.com/docs/hooks),
[Codex app-server](https://learn.chatgpt.com/docs/app-server).

## Remaining host and gate work

cmux again denied separate unfocused workspace creation before creating anything:
only processes started inside cmux may connect. Existing windows were untouched.
An allowed cmux-origin process is needed for the separate synthetic-provider demo.
Sidebar/tab/window agreement, latest-message line, herdr and LOO-428's configured
shim path remain unobserved. No publication, landing or acceptance is claimed.

Live lf rename changes cmux workspace/tab names only. OSC and Claude's native name
are set at launch; Codex receives terminal-title suppression. Whether host control
renames update the window bar remains unknown. Repair requires observed host
behavior; adding an unsynchronized terminal writer would violate native passthrough.
Visible agreement remains demo-owned, not waived by the source fixture.

Gate retains driver replacement, missing cmux executable, captured-Flow launch and
installation-entry acceptance. The earlier twelve-launch public CLI fixture covers
automatic Task/taskless titles, rename/reconnect and host failure in isolated homes;
only stderr has a PTY, so it proves no provider TUI or host rendering. Existing
lifecycle/capture/attribution checks cover human precedence and request/Goal identity.

## Delete — do not maintain

Random word lists and duplicate child wait loops are removed. Both native and lf
names use `request_title`; only `TerminalTitle` writes lf presentation. Preserve
native history, human names, lf attribution and host message hooks. No provider
wrapper, terminal registry, schema change or second naming algorithm is planned.

Review repaired native/lf naming competition and preserved symlinked provider
settings. Release's child memory reinforces testing each entry point: native
callback, actual hook, TUI reconnect and installed distribution are distinct proofs.
The shared-daemon probe does not repair embedded execution or synthesize an attach
hook. Dependent universal-native implementation requires a revised mechanism.

Checks: `cargo build`, focused Rust compilation, network-isolated title units (4) and CLI ownership (1), `native_titles.py` Claude/Codex naming/manual-rename/resume plus Claude unnamed-resume, fmt/Clippy, Ruff and `git diff --check` pass; Codex `--unnamed-resume` reproduces the missing callback; remaining acceptance is gate/demo-owned.

Earlier request/Goal and Task-primary proof: `91c4d07f9:scratch/every-lf-session-in-cmux.md`.
Earlier transport/demo plan: `04027f096:scratch/every-lf-session-in-cmux.md`.
Public cmux naming documentation and CLI help were studied; no cmux/herdr source,
tests, skills or configuration were read or reused.

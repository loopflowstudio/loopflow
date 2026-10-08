# Session titles (LOO-439)

## Accepted scope — October 8, 2026

Jack Heart requested implementation through demo, without provider accounts,
changes to existing cmux windows, or landing. His October 8 correction
`283fa7b5-3fdd-4457-9170-0de821534803` supersedes the earlier narrowed scope:
cover as many **future** Sessions as supported mechanisms allow, including
lf/plain, interactive/headless, Task/taskless and embedded Codex. Coverage takes
precedence over a single mechanism. No provider capability work is authorized.

Existing conversations are entirely out: no instruction-title repair or old
unnamed reconnect handling. Concurrent manual-rename preservation remains unproved.
The demo must show a short start-path coverage table, including unsupported future
starts and the exact missing capability. No question or scope decision blocks it.

Sidebar/tab/window agreement, readable Task-first titles, plain terminal/herdr
presentation and retained latest-message hooks remain host acceptance. Request
excerpts are the implementation choice; title quality needs Jack's demo judgment.

## Implementation and ownership

SQLite's Session name owns lf presentation. `engine::naming` uses a three-word
excerpt of the attributed request, excluding common articles and terminal controls,
within 80 characters. UserMessage assets can come from either channel; LOO-429's
generic trigger and inherited Goal are not naming inputs. Concrete skills, Task
titles and workspace names supply missing purpose. Task-primary Sessions use Task
titles. Existing names and human precedence survive. Generic opening words remain
a naming-quality limitation; this is an excerpt, not a semantic summary.

`TerminalTitle` serves both interactive entry points. Task presentation prefixes
the identifier and limits purpose to three words; taskless presentation uses the
stored name. OSC 0 is written before spawn. cmux workspace/tab updates use the
launching environment's identifiers. Read-only observation stops on driver
replacement; bounded host-command failure disables updates until reconnect.
Other terminals update on reconnect. Claude receives `--name`; titled lf launches
suppress Claude/Codex terminal-title updates. Native stdio and message hooks stay.

Plain native hooks share the excerpt function and run only on UserPromptSubmit.
Claude supplies `sessionTitle` while preserving nonempty `session_title`; this
works for new interactive and `claude -p` conversations. Codex uses `thread/read`
and `thread/name/set` on the existing shared app-server control socket. Neither
provider reads old turns or installs a resume hook. Both skip nonempty names and
lf-owned conversations. Providers own native history and terminal output.

Published installation merges UserPromptSubmit hooks under both standard homes.
Existing hooks, settings, permissions and symlink destinations survive. Hooks act
only beneath `.lf` directories. Codex's ordinary trust review remains. No native
settings were installed on Jack's machine. Installation-entry acceptance belongs
to gate; helper fixtures establish merging and repeated installation only.

## Demo coverage — October 8

| How the Session starts | Titled? | Evidence or missing capability |
| --- | --- | --- |
| lf interactive, Task | Yes: Task id and purpose | Public CLI/PTY fixture; live cmux rendering still unavailable. |
| lf interactive, taskless | Yes: Session name | Same fixture proves agreement with `lf session list` and live host rename commands. |
| lf headless, Task or taskless | Yes in Loopflow | Shared capture creation names every agent Session before provider launch; no terminal title without a terminal. |
| New plain Claude TUI | Yes | Real 2.1.295 TUI emits request title; manual rename survives the next turn. |
| New plain `claude -p` | Yes | Real 2.1.295 headless run saves `Plan store migration` in native history. |
| New plain Codex TUI, shared server | Yes, with trusted hook | Real 0.161.0 TUI emits request title using native title defaults. |
| New plain Codex TUI, embedded (`--no-daemon` / bypass-hook-trust) | No automatic native name from lf | No control socket to its active engine; inspected hook output has no title operation. |
| New plain `codex exec` | No automatic native name from lf | Real 0.161.0 headless fixture completes, leaves name unset and exposes no shared socket. |
| Plain native launch with hooks disabled/untrusted or outside `.lf` | No lf naming | Callback is not run or intentionally does not apply. |

Account-free TUI captures: `/tmp/lf-title-future-claude-tui-20261008/` and
`/tmp/lf-title-scope-codex-20261008/`.
Headless readbacks, stdout and stderr are in
`/tmp/lf-title-future-{claude,codex}-headless-20261008/` (`naming.json`). Fixed
fake responses establish transport/name persistence, not model judgment. The
headless Codex probe explicitly enables its known fixture hook; lack of trust is
not the missing endpoint. Future native coverage requires the published hooks;
no installed acceptance is implied.

The [Codex hooks](https://learn.chatgpt.com/docs/hooks) output contract offers
context/control, not a title field. The
[app-server API](https://learn.chatgpt.com/docs/app-server) offers `thread/name/set`
through a connected server. Inspected
[CLI commands](https://learn.chatgpt.com/docs/developer-commands) and local 0.161.0
`exec --help` expose no external embedded rename command. This audit identified no
supported native naming operation for the missing modes; it does not disprove
undocumented integrations. No provider change, storage writer, launch-mode switch
or competing OSC writer was introduced.

Existing-name and old-reconnect capability evidence remains at
`4b4aba7d9:scratch/every-lf-session-in-cmux.md` and its native probe. Additional
findings were preserved before reconciliation at
`/var/folders/m6/r3tllnrs1yq7yfbwm680tss40000gn/T/lf-title-scope-evidence-coy9w1oj/`.
These are historical evidence, not remaining implementation requirements.

## Evidence and remaining demo/gate work

`tests/e2e/native_titles.py` exercises new native conversations against a local
fake API in private PTYs or with `--headless`, fresh homes and external-network
denial. It retains native defaults, latest-OSC assertions and sequential manual
rename preservation. Old unnamed/named resume probes and all resume-only naming
code are removed.

The twelve-launch public CLI fixture covers Task/taskless names, rename/reconnect
and host failure in isolated homes. Only stderr has a PTY; this proves no native
TUI or rendered host. Lifecycle/capture/attribution fixtures retain human precedence
and request/Goal identity. Gate retains driver replacement, missing cmux executable,
captured-Flow launch and installation-entry acceptance.

The October 8 demo attempt again failed before creating its unfocused workspace:
the caller must originate inside cmux. A separate workspace with `--focus false` remains the only
authorized live host demo. Sidebar/tab/window agreement, latest-message line,
herdr and configured-shim behavior remain unobserved. Live lf rename updates
workspace/tab names only; window-bar propagation needs observation. No
unsynchronized terminal writer is planned. No publication, landing or acceptance
is claimed. Release's entry-point lesson applies: native callback, actual hook,
TUI/headless launch, installation and configured-host proofs are distinct.

## Delete — do not maintain

Removed both providers' SessionStart registration, resume transcript readers and
all old-conversation probes. Existing lf reconnect behavior remains part of the
Session lifecycle; this Task adds no historical repair. Future headless probes
replace the old-conversation branch. No provider capability work is planned.
Random word lists, duplicate child wait loops and capture-owned naming policy
were already removed. `engine::naming`, `TerminalTitle` and `harness::native_titles`
retain their separate naming, projection and native-hook responsibilities. No
wrapper, terminal registry, schema change or second naming algorithm remains.

Compression removes the probe's second Codex API client and WebSockets dependency;
native OSC output proves live adoption, while headless probes retain native-history
readback. Hooks derive one title before provider work; empty requests need no
database or socket. The interactive runner owns its observer directly. Review
also fixed an outside-repository assertion previously masked by the generic trigger.

Checks: `cargo build -p loopflow --bin lf`, isolated `native_titles` units (2), public CLI title/rename/reconnect fixture (12 launches), Claude/Codex TUI probes, fmt, all-target Clippy, Ruff and `git diff --check` pass; TUI captures: `/tmp/lf-title-compress-{claude,codex}-20261008/`; prior headless proofs retained at `b1feca0eb:scratch/every-lf-session-in-cmux.md`; host/installation acceptance remains demo/gate-owned.

Earlier request/Goal and Task-primary proof: `91c4d07f9:scratch/every-lf-session-in-cmux.md`.
Earlier transport/demo plan: `04027f096:scratch/every-lf-session-in-cmux.md`.
Public cmux documentation and CLI help informed behavior only; no cmux/herdr
source, tests, skills or configuration were read or reused.

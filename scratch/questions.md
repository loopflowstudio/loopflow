# LOO-593 decisions and open questions

Jack Heart selected direct server writes in [server-write-direction.md](server-write-direction.md).
This supersedes the full-CLI subprocess design. Accepted delivery remains the
existing feature Graph and PR #1813 through publication and interactive demo;
no merge, Auto-Ship or Task completion. Direct execution is committed at
`2d9c295d8a`; adoption/binding repairs are checkpointed at `b11d49d32f` and
main through #1822 at `30b4394a57`. The working design tracks remaining proof.

The [working design](mobile-backend.md) owns the deletion list, invariants and
remaining proof. LOO-718 owns remote admission, pairing and iOS; do not edit its
checkout. Installed-state migration keeps the old plist and stable socket/lock,
not a second Work implementation.

The [direct-write demo](direct-write-demo.md) records native Task creation,
conversation, rename and save; [matched measurements](server-performance/results.md)
record the direct-write comparison. Both predate the final repairs and main
merge. Merged-build validation and automated composed installed-helper replacement
passed as recorded in the working design. Interactive demo/review owns rendered
acceptance and released-version upgrade proof. Earlier
subprocess evidence cannot substitute. No matched embedded RSS sample supports
a memory-savings claim.

GUI-less distribution, before-login hosting, removal during active work and the
phone takeover gesture remain separate product decisions. No new assumption
about them is needed for the accepted local cutover. Session-to-Task adoption now
shares creation dispatch; ordinary create-and-run retains its execution owners.

## Continuation decision — November 4, 2026

Jack Heart authorized continuation after three usage-limit failures and newer
capacity evidence. The existing feature Graph continued through implementation
and sync; the former absence of a live Graph is historical. No credential or
account-routing change was selected. Original retry evidence and stopped Graph
IDs remain at `0e597c17a1:scratch/questions.md`. No additional product decision
is required for the local cutover; final acceptance is still outstanding.

Settings-merge assumption (November 4): file settings retain access before Work
is compatible. The additive status/configuration operations preserve existing
bootstrap fields and route the incoming FFI consumers through the shared server.
This is an implementation choice within Jack Heart's complete FFI deletion.

## Revision-2 follow-ups — November 5, 2026

Jack Heart approved seven protocol follow-ups after reviewing revision 2. Choices
made while delivering them, for Jack to confirm or reverse:

- **Checkpoints carry no argument changes.** The shared watch's stdin takes
  `{"id":n,"args":…}`, but `args` is each watch's private argument type
  (`SessionWatchArgs`, `TaskStatusArgs`, `HistoryWindow`), not the served read's
  shape, and no client sent one. `checkpoint {id, checkpoint}` is the only
  subscription message; new arguments are a new subscription, whose first
  reading is complete anyway. The CLI's `--watch` stdin keeps `args`.
- **Server-owned kinds.** Jack's mapping covers the shared reads and writes.
  The three server-owned operations took `session.owner` (a Session's owner
  Process), `config.file` (the settings-file API: catalogue, declarations,
  explanations, themes) and `terminal.attach`. A subscription is its own
  message, `subscribe {id, operation}`, carrying a read; `request` carries
  anything.
- **`task.list` takes `closed: true` only.** No open-Task list exists apart from
  `plan.get`; `closed: false` answers `invalid` naming `plan.get`.
- **`session.list` folds the whole list and its pages.** `limit` makes it a page
  (`after` needs a `limit`); only the whole list has a watch.
- **Wire errors for an unknown kind are generic.** Serde's untagged dispatch
  answers `invalid: data did not match any variant…`; a named-kind error would
  need a hand-written deserializer.

- **Typed writes keep one CLI spelling, for SSH and Session preparation.**
  `task.claim`, `task.unassign`, `task.hold`, `session.resume` and `session.new`
  still build an `lf` spelling (`Write::cli`), because their Work can live on
  another Machine and SSH runs the destination's ordinary `lf … --json`, whose
  output is that Machine's API value. Session preparation also reads its
  selectors (`--agent`, `--task`, `--wave`, `--repo`) through the parsed Cli
  (`SessionConnectionArgs::target`); typing that resolver is a deeper refactor
  of `session.rs`. No other write parses or prints a CLI spelling.
- **CLI gained `--json` where a write had only text.** `lf task complete`,
  `task reopen`, `task edit`, `task move` (membership), `ship --stop` and
  `graph customize` now print with `--json` exactly what the server returns:
  the Task, the Task edit (`{id, sync}`), `{issue, stopped}` and `{path}`.
  `task.abandon` returns the identifier string, as `lf task abandon --json` did.
- **Unnamed write failures are `outcome_unknown`.** A write's error names its
  code when the shared code raises `wire::ServerError` (an empty Session name is
  `invalid`) or when its type implies one (`StoreError::NotFound` →
  `not_found`, the new `StoreError::Stale` for Dashboard revisions →
  `conflict`, selection and JSON errors → `invalid`). Anything else may have
  partially applied (planning sync, SSH), so it is `outcome_unknown`. Naming
  more codes means typing more of `OpsError`, which today is mostly strings.
- **Write diagnostics live in the log, not the response.** Notes the shared
  functions print to stderr ("Saved locally; pending Linear sync", the bound
  conversation's directory) go to `logs/lf.log` under the request's trace.
  The values carry the same facts where they matter (`cwd`, `sync`).
- **`desktop.portfolio.set` takes a typed `change`.** `PortfolioCommand` now
  derives serde, tagged by `action`; Swift maps its word list to it.

### CLI paths that differ from their API name

Input for the follow-on Task that aligns CLI, config and API namespaces. Kinds
whose CLI path already matches (`task.complete` = `lf task complete`, also
`task.reopen`, `task.abandon`, `task.move`, `task.claim`, `task.unassign`,
`task.save`, `task.diff`, `task.file`, `session.rename`, `session.bind`,
`graph.list`, `graph.customize`, `machine.list`, `wave.list`,
`history.retention`) are left out. `session.resume` and `session.new` match
apart from `--prepare`.

| API kind | CLI today |
| --- | --- |
| `task.create` | `lf task create --session <id> --title <title>` |
| `task.update` | `lf task edit <issue> --notes=<notes>` |
| `task.hold` | `lf --task <issue> ship --stop` |
| `task.get` | `lf desktop task <issue>` |
| `task.list` | `lf desktop done --wave <wave>` |
| `task.changes` | `lf task diff <issue> --files` |
| `task.context` | `lf usage --days 0 --task <task> --context` |
| `task.runs` | `lf graph list --processes --for-task <task>` |
| `session.list` | `lf desktop sessions [--page --limit n]` (`lf session list` is a sibling) |
| `session.bind` with `preview` | `lf session bind --dry-run` |
| `session.owner` | none |
| `wave.get` | `lf desktop wave <wave>` |
| `wave.create` | `lf wave add <wave>` |
| `wave.graph` | `lf wave graph show <wave>\|--no-wave` |
| `wave.sync` | `lf wave status <wave> --sync` |
| `chapter.start` | `lf wave new-chapter --all <name>` |
| `plan.get` | `lf desktop plan` |
| `repository.planning` | `lf config planning status` |
| `repository.people` | `lf ops people` |
| `repository.tokens` | `lf ops tokens` |
| `repository.path` | `lf --repository <id> config repository --path` |
| `graph.run` | `lf graph show <id> --processes` |
| `machine.self` | `lf machine id` |
| `machine.doctor` | `lf doctor` |
| `process.list` | `lf monitor ps` |
| `usage.get` | `lf usage` |
| `history.list` | `lf history` |
| `work.resolve` | `lf open <url> --link` |
| `config.settings` | `lf config show` |
| `config.file` | the `lf config` setting commands (`config::api`) |
| `view.list` | `lf desktop plans` |
| `view.get` | `lf desktop view [expression] --plan` |
| `terminal.attach` | none; the CLI reaches an owner's socket directly |
| `desktop.portfolio` / `.set` | `lf portfolio [<change>]` |
| `desktop.layout` / `.save` | `lf desktop layout <work> [--save]` |
| `desktop.dashboard.query` | `lf desktop query` |
| `desktop.dashboard.pin` / `.unpin` | `lf desktop pin` / `unpin` |
| `desktop.dashboard.rename` | `lf desktop rename query:<id>` |
| `desktop.dashboard.hide` / `.show` | `lf desktop hide` / `show -p query:<id>/<member>` |
| `desktop.dashboard.slots` | `lf desktop layout query:<id> --save` |
| `desktop.dashboard.move` | `lf desktop move -p <board>/<member>` |

### Item 7: one terminal screen format — stopped, nothing to unify on the wire

The viewer already receives one format. A live attach's `0x02` frame and an
ended Session's preview are both Ghostty VT formatter output
(`transport::terminal::repaint`): `Output::snapshot` repaints the owner's live
`ghostty_vt::Terminal`, and `transport::snapshot::last_screen` decodes the saved
`GHOSTSNP` stream back into a `Terminal` and repaints it the same way. `GHOSTSNP`
is only the owner's storage format on disk.

Rex can send its snapshot as a client's initial screen because every Rex client
embeds libghostty-vt and owns the displaying terminal's state. Ours do not:

- The CLI viewer writes to whatever terminal emulator the person runs (iTerm,
  Terminal, tmux, a Desktop pane). Those take VT bytes, not `GHOSTSNP`.
- Desktop's panes are GhosttyKit app surfaces running the `lf` relay. The
  bundled GhosttyKit header (`ghostty.h` in the GhosttyKit xcframework) has no
  call that restores a surface from a snapshot stream, nor one that feeds bytes
  into a surface; a surface only reads its child's PTY.

Sending `GHOSTSNP` as the live initial screen would need, for Desktop, a
GhosttyKit surface API that restores terminal state from a snapshot (something
like `ghostty_surface_restore_snapshot(surface, ptr, len)`, built from the same
Ghostty revision as `rust/loopflow/ghostty-vt.json` so formats match), plus a
viewer path that hands the frame to it instead of the PTY. The CLI viewer would
still have to decode and repaint through the formatter for a plain terminal, so
VT formatter output remains the one format every viewer can display. The owner
protocol framing is unchanged.

## Engine — November 5, 2026

Step 1 (task.* and the core loop). Decisions made headless; each is reversible.

- **Where the run lives.** A Task run outlives its request and needs a
  registered Process to carry it (`task_place` records the carrier; retries
  live in `run_task_graph_for`). The server has no Process and must not hold
  the run, so `engine::task::run` and `::ship` start the same command a
  terminal would (`lf [-b] [-a <agent>] task run <issue> [--from <step>]`,
  `lf --task <issue> ship`) under its own execution owner via
  `os_process::start_lf_session`, the path `restart_ships` already uses, and
  read the typed value back from a private report file the command writes once
  (`LF_LAUNCH_REPORT=<path>`, taken and cleared on startup of the command). That is a process launch, not an argv round
  trip: nothing parses stdout, and the CLI's `--json` print and the report are
  one value (`engine::task::TaskRun`, `TaskShip`). Open: if Jack wants no
  carrier process at all, the run has to become a server-owned job, which the
  server README currently rules out ("creates no … Graphs").
- **Harness when the request names none.** `task.run` with `agent: null`
  passes no `--agent`; each step resolves its harness as `lf task run` does
  (the step's/Wave's/repository's configured default, else the builtin).
  Jack's rule that Tasks run on Claude is a setting on his repositories, not a
  server default; a client that must pin it sends `agent: "claude"`.
- **Auto-Ship without `approve`.** `approve: false` is bare `lf task run`: it
  stops before the next interactive step, but a graph with none ahead reaches
  `end`, and reaching `end` approves Auto-Ship (the documented rule). A client
  that must never approve should check `task.get`'s graph first, or Jack may
  want `task.run` to refuse when no interactive step lies ahead.
- **Placement.** The served `task.run` skips `--pool`/`--rent`/`--quote`
  placement; the carrier's ordinary dispatch still applies default placement.
- **Remote Tasks.** `task.run`, `task.ship`, `task.hold`, `task.claim` and
  `task.unassign` route through `work_route::remote_task` + `run_remote`
  (typed route lookup; Clap is parsed only for the SSH spelling). For
  `task.run`/`task.ship` the destination's `lf … --json` delegates into an
  owner and relays its output over SSH; the value parses only if nothing else
  reaches stdout first. Untested against a real second Machine.
- **`session.resume` / `session.new`** keep `Write::cli` for SSH routing and
  Session preparation; they belong to step 4's `session.*` pass.
- **`task.comment` result.** It returns the thread after the save
  (`TaskComments`, what `lf task comment --json` prints), not only the saved
  comment; the saved one is the last entry. A narrower `{comment}` result is a
  one-line change if a client wants it.
- **`task.wait` output.** `lf task wait --json` now prints the reading
  `{reached, task}` (it printed the bare snapshot), so a timed-out wait says it
  did not reach its state. The served subscription ends with a new `ended
  {id}` message; Swift's `ServerClient` finishes the stream on it.
- **Read purity.** `task.get` (`desktop task`) ran `task_status`, which syncs
  the repository with Linear and inspects unknown issues; it now reads the
  store (`read_task_status`). `wave.get` and `plan.get` still sync; they are
  `wave.*`, not this step.
- **Errors.** `wire::ServerError`/`ErrorCode` moved to `engine::Error` /
  `engine::ErrorCode`; the wire carries them. A refused detached run (for
  example a Task whose planning cannot launch) answers `outcome_unknown` with
  the carrier's message, as any untyped write failure does.
- **Desktop callers.** `WorkModel.shipTask` and `runTask` still spawn `lf`;
  moving them to `task.ship` / `task.run` needs `task.run` to take a `graph`
  and the headless test that taps the ship argv to change.

Step 2 (session.asks and session.answer) was reverted on Jack Heart's direction:
the phone gets the real terminal on the real Claude Code session, so there is
no asks reading and no answer write.

Step 3 (namespace alignment). Decisions made headless; each is reversible.

- **Config is one path.** `config::api` moved to `engine::config`; its
  `Request` enum and the server's `config.file` operation are gone. The kinds
  are `config.catalog`, `config.settings`, `config.get`, `config.explain`
  (reads) and `config.set`, `config.unset`, `config.theme` (writes), each
  spelled by `lf config catalog|settings|get|set|unset|explain`. `lf set` and
  `lf unset` stay the everyday shorthand (reserved owner). `config.theme {path,
  value}` has no CLI: Appearance owns an arbitrary theme file and its
  Apply/Cancel transaction; reading is `value: null`. If Jack wants theme to be
  an ordinary `config.set` on the personal global destination, Appearance's
  file choice has to go.
- **Wave's plan.** `wave.get` (Desktop's one-Wave read) is renamed
  `wave.planning`; `plan.get` (every Wave's plan) is `planning.list`.
- **Chapter rotation's engine stays.** `lf wave new-chapter`, the
  `chapter.start` write and Desktop's "Realign Projects…" sheet are deleted;
  `ops::chapter::{new_chapter, rotate}` and the store's project rotation keep
  their tests but no command reaches them. Delete them when Jack decides KRs
  are not returning, or rewire when they do. The `start-chapter`,
  `wave_start-chapter`, `review-chapter`, `split-wave` and `init` skills still
  describe `lf wave new-chapter`; they need Jack's call on what chapter
  planning becomes before they are rewritten.

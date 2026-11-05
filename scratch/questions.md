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

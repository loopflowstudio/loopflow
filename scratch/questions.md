# 5b3da9 open questions

- `lf history --watch` now waits up to 5 minutes before an entry ages out of `--since`. New activity is still read as soon as it is committed.
- The Working Tasks sidebar gap (a started Task missing while `lf desktop view --plan --json` returned it) can't be checked headless. Desktop's `planning` part (`lf desktop plan --all --watch`) is the reader to check during demo.
- Read/write classification judgment calls (`Commands::changes_state`): Desktop UI actions (focus, send, zoom) record a Process, since they act on terminals; `lf wt switch` and `lf ci` record none; bare `lf pr`, `release notes` and `planning status --watch` record. Each is one match arm to flip.
- `Operation::PlanningStatus` runs the planning exchange in-process for as long as Desktop subscribes, as `lf planning status --watch` does. That makes the app a store writer, which the design lists as open (app and CLI `lf` both writing, migrations, version skew). The simpler alternative is serving only the reading and leaving the exchange to an `lf` process. It is one match arm.
- `Operation::DesktopTask` requires a directory. Desktop's `task` part runs without one when its scope has no repository; the Swift move passes the repository, or the Task's checkout, when it builds the Operation.

# 5b3da9 open questions

- `lf history --watch` now waits up to 5 minutes before an entry ages out of `--since`. New activity is still read as soon as it is committed.
- The Working Tasks sidebar gap (a started Task missing while `lf desktop view --plan --json` returned it) can't be checked headless. Desktop's `planning` part (`lf desktop plan --all --watch`) is the reader to check during demo.
- `.lf/wave/infrastructure/MEMORY.md` is 332 tokens over its 16,000-token target. Jack Heart (comment 425c8035): leave it alone; Wave memory is read-only for this Task.
- Read/write classification judgment calls (`Commands::changes_state`): Desktop UI actions (focus, send, zoom) record a Process, since they act on terminals; `lf wt switch` and `lf ci` record none; bare `lf pr`, `release notes` and `planning status --watch` record. Each is one match arm to flip.

Commands that ran now appear as processes in help, the Task view, and current documentation. The same name carries through Rust, Swift, JSON and SQLite, while recorded identities, ancestry and exit evidence stay intact.

## What changes

- Replace Exec/ExecId and FlowExec with Process/ProcessId and FlowProcess; move existing consumers and fixtures together, without alternate commands or owners.
- Add one migration that renames tables, columns, indexes, triggers and the revision domain while preserving populated history and constraints.
- Preserve released migration bytes and dated records. Captures, process receipts, retained provider environments and append-only decisions keep the decoding needed for replay and recovery.
- Rename the identity flag to `--accept-unknown-process`; command placement remains with LOO-397.

## Checks

Rust migration, discovery, Flow history, journal, retained uncertainty/capture, DTO, ownership and observation checks passed. All-target check and Clippy, formatting, architecture and documentation synchronization passed. The headless Desktop selection passed 42 tests.

A disposable Home confirmed that `lf help --all` contains no Exec and `lf monitor show ID --json` returns `parent_process_id` and the recorded exit code. Installation-only proofs remain with isolated CI; no installed migration or full gate is claimed. HTML rendering is unavailable in this session.

## Try it

Inspect a recorded command with `lf monitor show ID --json`, or open a Task's recorded work in Desktop. The command has a process identity, its parent when known, and its exit status. The CLI inspection was observed locally; the Desktop interaction is a suggested review backed by headless view checks.

Jack Heart requested publication through pr-review. This change stops there.

# Linear-owned PM

## Decision

Linear is the sole durable source for projects, their definitions and KRs, and
tasks. A wave keeps only its objective and its stable Linear Initiative binding
in `GOAL.md`. There is no `wave/<wave>/projects/` representation.

The machine SQLite registry is a read model, not another authoring surface.
`lf pm sync` fetches each linked Initiative, its Projects, and their Issues and
atomically replaces that wave's snapshot. Ordinary reads never call Linear:
`lf pm show`, project selection, agent context, and the Mac read the snapshot.

## Contract

- `lf pm init [--all]` links or creates Initiatives only. A missing binding is
  resolved by one exact Initiative-title match, then written to `GOAL.md`.
- `lf pm sync [--wave <wave>]` is the explicit network refresh. It stores full
  Project content, including definition and KRs; Linear's 255-character
  description remains only a short summary.
- `lf pm show [--wave <wave>] [--project <slug>] [--json]` reads SQLite. Its
  JSON includes Projects and their tasks so it is the export for LLM and UI
  consumers. A missing snapshot tells the user to run `lf pm sync`.
- PM mutations write Linear, then refresh the affected wave snapshot before
  returning. They do not create markdown.
- `lf pm doctor` may query Linear because its purpose is to compare the remote
  system with local bindings and snapshots.
- Project creation and editing use `lf pm project create/update`; they write
  Linear and refresh SQLite without recreating a file bootstrap path.

## Storage

Store one JSON snapshot per `(repo, wave)` in SQLite with provider, Initiative
id, and `synced_at`. The JSON uses the same typed PM domain structs returned by
the provider. Replacing one row makes a refresh atomic and keeps the schema
small; Projects and Issues are not independently queried by SQLite.

## Removal

Delete local-project parsing, seed markers, markdown cache rendering, stale-file
reconciliation, all project directories, and docs/skills that instruct agents
to edit them. Reroute the Swift plan loader and project promotion to `lf pm
show --json` before deleting their file readers.

# Linear-owned PM

## Decision

Linear is the sole durable source for projects, their definitions and KRs, and
tasks. A wave keeps its identity and stable Linear Initiative binding in
`GOAL.md`; it does not keep a project representation there or under
`wave/<wave>/projects/`.

The machine SQLite registry is a read model, not another authoring surface.
`lf pm sync` fetches each linked Initiative, its Projects, and their Issues and
atomically replaces that wave's snapshot. Reads serve that snapshot and only
reach Linear through a bounded staleness policy (see Freshness policy):
`lf pm show`, project selection, agent context, and the Mac read the snapshot.

## Contract

- `lf pm init [--all]` links or creates Initiatives only. A missing binding is
  resolved by one exact Initiative-title match, then written to `GOAL.md`.
- `lf pm sync [--wave <wave>]` is the explicit network refresh. It stores full
  Project content, including definition and KRs; Linear's 255-character
  description remains only a short summary.
- `lf pm show [--wave <wave>] [--project <slug>] [--json]` reads SQLite through
  the freshness policy. Its JSON includes Projects and their tasks so it is the
  export for LLM and UI consumers. `--sync` forces a refresh first; `--no-sync`
  never touches the network (cache-only). A missing snapshot under `--no-sync`
  tells the user to run `lf pm sync`.
- PM mutations write Linear, then refresh the affected wave snapshot before
  returning — every one of them. They do not create markdown. Consequence: on a
  single machine the snapshot runs *ahead* of its last explicit sync, because
  each local change already folded into it. A `--no-sync` read there is fully
  current without ever calling Linear.
- `lf pm doctor` may query Linear because its purpose is to compare the remote
  system with local bindings and snapshots.
- Project creation and editing use `lf pm project create/update`; they write
  Linear and refresh SQLite without recreating a file bootstrap path.

## Freshness policy

`pm show` (Auto mode, the default) reads the snapshot through a staleness policy
keyed on `synced_at`. Because every mutation refreshes the acting machine's
snapshot, reads are usually fresh and never touch the network. Distributed
change — other machines, the Linear web UI — is caught by a bounded opportunistic
refresh:

- **fresh (< 1 hour):** serve the cache, no network.
- **soft-stale (1 hour – 1 week):** try one refresh (5s cap); on any failure
  (timeout, offline, auth), fall back to the cached snapshot and say so.
- **hard-stale (> 1 week):** try one refresh; if it cannot reach Linear, error.
  A week-old snapshot is too stale to serve silently.

`--sync` forces a refresh regardless of age; `--no-sync` skips the network
entirely. The refresh is best-effort and bounded — no single-flight lock; a
stale storm is acceptable. Every human `show` prints the snapshot age so
staleness is never silent.

The policy is uniform — agents run the same `lf pm show`, with `--no-sync` for
deterministic, network-free reads. The agent loop stays robust not by a special
read mode but by tolerating failure: if the command cannot produce data, the
caller drops the PM section and proceeds rather than blocking or crashing.
Keeping the snapshot warm for cross-machine readers is the scheduled
`lf pm sync` cron's job.

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

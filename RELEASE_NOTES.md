# v0.14.4

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.4 keeps repository Work and native Sessions connected when a checkout moves or its directory is renamed. Loopflow retains the repository’s identity and updates its current address, so Desktop navigation and Session recovery can follow the move without losing Task membership or native conversation identity.

## Keep working after a directory move

A moved checkout previously received a new repository identity, leaving Desktop navigation and Session callbacks pointing at the old address. Loopflow now retains identity in the Git common directory and reconciles current addresses together, while Desktop refreshes the existing workspace without replacing Session owners. (#1578)

- Preserve historical paths, Task membership, native conversation IDs, sibling worktrees and unpublished edits.
- Distinguish a repository move from an independent clone or a different checkout reusing an old path through machine-scoped location records.
- Resume native conversations after a rename; newly launched provider callbacks resolve relocated references from their recorded launch information.

## Operational notes

- Existing checkouts without a location record have explicit recovery through the `repo identity` command’s `--bind` and `--relocate` options. (#1578)
- Already running older providers retain their existing hook environments. The callback change does not rewrite those live environments. (#1578)
- The PR records repository preservation, native resume, callback, migration and Desktop navigation checks. Coordinated release/install and acceptance with the installed CLI and Desktop remained outstanding at implementation delivery; the Docker installation harness was unavailable locally. (#1578)

## Small changes

- Disposable data clones remain isolated from repository relocation. Historical linked-checkout registrations preserve unrelated checkout context and Task placement without writing a location record into the main Git directory. (#1578)
Read when the change moves crates or build scripts.

# Build and crate changes

When the change moves crates or build scripts, the people it is for are
developers building in many worktrees at once. Build twice with nothing changed:
the second must print no `Compiling`. If it does,
`CARGO_LOG=cargo::core::compiler::fingerprint=info cargo build …` names the
stale input. A `rerun-if-changed` on a directory is scanned recursively, so a
build script watching the shared `.git` rebuilds on every worktree's commits; a
harmless `git hash-object -w <tmpfile>` reproduces that. Then grep for every path
the branch moved (`git diff -M --name-status --diff-filter=RD origin/main...`)
outside Rust sources: Swift tests, `swift/ios.yml`, scripts and docs read
fixture and source paths directly. Each fresh `cargo build` re-links
`target/debug/lf` to a new inode, and its first exec spends 1–3 s in
`_dyld_start`; time the second run when timing `lf` itself. Served `task.get`
takes `directory` along with `issue`.

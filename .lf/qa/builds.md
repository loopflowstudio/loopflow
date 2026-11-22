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

# A crate move that should change no behavior

Compare the branch `lf` with the installed one on the same private home
(`.lf/qa/server.md`), real `HOME` so the ambient provider logins answer.
Record with the installed `lf`: `lf -b -a <harness> : '<prompt>'` per harness,
plus one interactive `lf -a claude : '<prompt>'` in a PTY. Then run every read
with both binaries into two directories and `diff -r` them: `session history`,
`session history <id> [--json]`, `session history show <id> [--json]`,
`session history search <word>`, `session usage [--json]`, `session retention`,
`session card <id>`. The only expected difference is the `LF_BIN` path in a
card's `open_argv`. Then launch each harness with the branch and resume the
interactive chat with it (`lf session resume <id>` in a PTY).

A `-b` batch is a Run: `session resume` refuses it on both builds. A chat with
no Task never shows in `session list`; read it live with `session card <id>`
(`liveness`, `session.turn`) and find its ID in `agent_sessions`. For a prompt
that calls a tool, OpenCode's and Pi's default models can fail upstream:
`-a opencode:opencode/big-pickle` and `-a pi:gemini-3.8-flash` answered.
Codex over its usage limit still records a failed turn with the provider's
error, which checks the error path and nothing else.

For the developer loop on the moved crate, change one compared string in a
decoder, run `cargo test -p <crate> <module>` (a test must fail, in seconds),
`git checkout` the file and run it again.

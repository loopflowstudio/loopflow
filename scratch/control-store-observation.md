# Installed control store mismatch

2026-09-28 · Supervisor observation during LOO-298 implementation.

Installed `lf task status LOO-298 --json` and `lf status infrastructure --json`
failed opening
`/Users/jack/.lf-dev/installed/local-afee63d734c7482cb94d1071af26d9ea/loopflow.db`:
`installed development store is incompatible (candidate removed or canonicalized
an applied draft)`. The first failing status was observed around 18:05 UTC.
Earlier status reads in this same implementation were successful.

Read-only SQLite inspection found `development_migrations` now contains
`auth_browser_bindings`, ID `2cd101ad721f80ca7903d743acc094c4`, checksum
`7118e30269767dbe7636f7c3957a288655ce8170af6552aea049f6d2b59924d0`,
applied at Unix time `1790618661`. This is not the new `record_execs` draft.
The responsible writer has not been established. No attribution to another
person or Task is inferred.

The installed immutable CLI still hashes to
`f5ef8d640340e9f8b9e36d17d84de83e14e905305c43e959fd00c49a322a527f`;
the checkout's `target/debug/lf` has a different inode and digest.
The worker remains live: Exec `60cc4bdb-6d29-481f-b5a7-de4b7d74b57d`,
PID 37099, provider PID 38334, Run `run_8e4800ebef6045699e0be1fac668e6cb`.
Installed `lf runs <id> --events` and `lf ps --json` still observe it.

Continue authorized local implementation and disposable proofs. Do not repair,
remove draft evidence, promote or run the branch binary against that Home.
Task control and saved-Flow settlement need rechecking before their next use;
an elapsed timeout or absent Task status is not permission to start a replacement
writer. This incident does not change the accepted product model or reduce the
code-complete finish line. No installed files or database rows were changed by
this inspection.

Jack subsequently reported that `lf` was fixed. Fresh installed
`lf session list --json` and `lf task status LOO-298 --json` both succeeded;
the Task still names the same Codex implementation Run. No replacement worker
was launched. This verifies restored reads, not the repair mechanism or the
eventual saved-Flow settlement. The inspection did not establish how Jack
repaired the installation.

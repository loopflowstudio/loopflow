# LOO-805: Git read retention

Scope retention to subscription readings. Served one-shot reads, including
`task.changes`, consult Git afresh while subscriptions remain active. Keep shared
subscription reads and side-by-side checkout listing fast. No open product question.

Delete — do not maintain (removed; no remaining targets):
- Process-wide retention activation in `Reader::new` and `git::retain_reads`.
  Keep the shared answer map, refresh-on-change and parallel checkout reads.
- `journal::request`'s write-only Git bypass flag. Positive subscription scope
  makes both ordinary reads and writes fresh by default.

The implementation scopes retention to each checkout subscription reading and
inherits it in bounded checkout/detail workers and nested request workers.
Starting another subscription does not clear shared answers. The server README
keeps the freshness contract; its comparison example uses supported `parent`.

Keep retention tests beside their owner: Git covers thread scope, refresh and
unwind separately; request workers share one runtime to exercise retention
followed by a fresh request on the same pool. Detail workers capture configuration
before wrapping Git scope, keeping the read loop flat.

Remaining: gate, review, QA and delivery through the existing Graph; sync before
delivery. Gate coverage: `test(git::tests::) | test(journal::request::tests::) |
test(lf::commands::subscribe::) | test(lf::commands::waves::) | binary(served_reads)`.

Docs presentation: `lf open --host desktop --file rust/loopflow/src/server/README.md`
failed because this Task has no workspace in the attached Desktop window; no
presentation changed.

Check: `TMPDIR=/private/tmp cargo test -p loopflow --lib --test served_reads subscription_git_retention` (inherited `LF_*` unset) — 7 passed after compression; macOS linker emitted its unwind-table size warning. `cargo fmt` and `git diff --check` passed. Clippy and broader affected suites remain with gate.

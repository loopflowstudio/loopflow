# LOO-845

Doc changed: `AGENTS.md` (Goals > Simplicity, new last paragraph). Prose only; no tests.

Check: `cargo test -p loopflow --test integration documented_commands::` — 4 passed (implement); gate reruns it.

Compress: nothing to reduce; the paragraph adds proactive cleanup, which "earn its place" and "a redesign deletes" don't cover. Open tension: builtin `rust/loopflow/src/builtins/task/skill/compress.md:12` says "don't ... wander into unrelated cleanup"; builtins run in other repos, so left as is.

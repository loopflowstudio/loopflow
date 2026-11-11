# LOO-845

Doc changed: `AGENTS.md` (Goals > Simplicity, new last paragraph). Prose only; no tests.

Check: `cargo test -p loopflow --test integration documented_commands::` — 4 passed (implement); gate reruns it.

Compress: nothing to reduce; the paragraph adds proactive cleanup, which "earn its place" and "a redesign deletes" don't cover.

Concept review: no change. The builtins' scope rules (`compress.md:12` "don't ... wander into unrelated cleanup", `gate.md:82`, `implement.md:80` "Stay in scope", `pr-publish.md:21`) bound *unrelated* work and new outcomes; the AGENTS.md paragraph covers code the change passes ("you pass", "you notice"), which those skills already count as related. New outcomes still go in their own Task. Builtins run in other repos, so they stay as is.

QA: paragraph reached a real lf-launched Claude Session via AGENTS.md; no mirrors to sync; nothing fixed (`scratch/qa.html`).

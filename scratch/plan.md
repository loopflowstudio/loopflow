# LOO-845

Doc changed: `AGENTS.md` (Goals > Simplicity, new last paragraph). Prose only; no tests.

Check: `cargo test -p loopflow --test integration documented_commands::` — 4 passed (implement and gate).

Compress: nothing to reduce; the paragraph adds proactive cleanup, which "earn its place" and "a redesign deletes" don't cover.

Concept review: no change. The builtins' scope rules (`compress.md:12` "don't ... wander into unrelated cleanup", `gate.md:82`, `implement.md:80` "Stay in scope", `pr-publish.md:21`) bound *unrelated* work and new outcomes; the AGENTS.md paragraph covers code the change passes ("you pass", "you notice"), which those skills already count as related. New outcomes still go in their own Task. Builtins run in other repos, so they stay as is.

QA: paragraph reached a real lf-launched Claude Session via AGENTS.md; no mirrors to sync; nothing fixed (`scratch/qa.html`).

Gate: reviewed against checkout base `1570bef610`; no content fixes. Changed paths: AGENTS.md and scratch notes. Architecture checks cover repository ownership; documented_commands covers maintained command examples. No runtime or UI changes.

Gate checks (inherited LF variables unset, stdin closed):
- `uv run python scripts/test.py --reuse-passing --rust-filter 'test(documented_commands::)'` — stopped before suites: uv-cache inventory exceeded its 15-second deadline; selected checks run directly instead.
- `uv run python scripts/check_architecture.py` — passed all seven categories.
- `cargo test -p loopflow --test integration documented_commands::` — 4 passed; 1m19s build, 6.85s tests.
- `uv run python scripts/check_rust_test_inventory.py` — passed.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy --all-targets --jobs 4 -- -D warnings` — passed.

Gate diagnostic exposed an internal LF_CAPTURE_KEY in process arguments in the tool log; rotate the affected session token. No value copied here.

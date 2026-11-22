# What each harness writes

A provider changed its output. Fix the decoder and run its tests:

```sh
$EDITOR rust/loopflow-harness/src/claude_decode.rs
cargo test -p loopflow-harness claude_decode
```

That builds this crate and `loopflow-model`, over `rusqlite`, `regex`, `serde`
and a few other registry crates. Storage, execution and the application are
not compiled, and a storage or execution edit does not rebuild this crate.

Reading a saved conversation needs no store either:

```rust
use loopflow_harness::transcript::{self, Source};
use loopflow_harness::HarnessKind;

let turns = transcript::read(HarnessKind::Claude, &Source::File(path))?;   // Vec<transcript::Turn>
```

## What lives here

The crate answers one question: what does each native client (Claude Code,
Codex, OpenCode, Pi, Antigravity) write, and where?

| | |
|---|---|
| `HarnessKind`; `Claude`, `Codex`, `OpenCode`, `Pi`, `Antigravity` | which client; one unit struct each |
| `Harness` | what a client declares about itself: the commands that are it (`program`, `recognizes_driver`), where it keeps transcripts, how long it keeps them, its model names and sign-in advice |
| `{claude,codex,opencode,pi,antigravity}_decode`, `fact`, `lf_tag` | a live stream, decoded into facts |
| `transcript` | a saved conversation read into `Turn`s: `read`, `find`, `scan`, and `read_current` for the turn in progress |

It never opens Loopflow's store, starts a process or reads the process table.
OpenCode's reader opens OpenCode's own SQLite file. Launching a harness,
recording it and joining a transcript to a Session are
[execution](../loopflow-execution/src/harness/README.md)'s, which implements
`Launcher` on the same five structs.

One thing here is Loopflow's own writing, not a client's: `transcript::Launch`
and `lf_tag` parse the `<lf:skill:NAME>`, `<lf:message>` and
`<lf:suggest_actions>` tags that execution's `prompt` writes into a
conversation or asks the agent to emit, so a reader can tell injected text
from typed text. The tag strings are literals on both sides; change them
together.

A function that needs a `SqliteStore` stays in execution and calls down. This
crate gains no workspace dependency to keep a file whole:

```sh
cargo tree -p loopflow-harness -e normal,dev | grep -cE 'loopflow-(storage|execution)'   # 0
```

## Fixtures

Captured provider output is under `src/testdata/` (streams) and
`src/transcript/testdata/` (saved conversations). A decoder or reader change
adds the output that prompted it. Tests that push decoded facts into a store
(`harness::conformance_tests`) and the launch matrix are execution's; they
replay the same streams through `fact::trace` and read the same saved
conversations through `transcript::testdata`, both exposed by the
`test-support` feature. `pi_rpc.py`, a stand-in Pi process, stays with execution's launch
fixtures.

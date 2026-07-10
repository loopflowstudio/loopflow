# One human thread command

## Receipt

On 2026-07-10, `lf chat --help` and `lf wavechat --help` expose two commands
for the same served wave thread. `docs/lf.md` documents both. This breaks the
Wave Chat proof that `lf chat` is the sole CLI owner of the human thread and
reintroduces a second product concept beside the thread/bus split.

## Contract

- `lf chat [TEXT]` remains the one-shot human message/steer door. With no text,
  it continues to read stdin so prompts and heredocs do not change.
- `lf chat --follow [-w WAVE]` replays and follows that same thread while typed
  lines post through the same unattributed `/messages` door.
- `--steer` with `--follow` makes typed lines request live steering; otherwise
  they queue as normal messages. `--parent` remains incompatible with steer but
  may be followed as a regular thread.
- `lf wavechat` is removed, not retained as an alias. Internal SSE rendering
  remains `thread`; the agent bus remains exclusively `lf radio` / `lf sub`.
- `--follow` conflicts with positional TEXT. This keeps one-shot input and an
  interactive session visibly distinct without inventing another noun.

## Ownership

`chat.rs` owns target resolution, posting, and interactive composition.
`thread.rs` owns only replay/follow rendering from the listener's SSE wire.
No wire shape, process lifecycle, journal semantics, or Swift surface changes.

## Proof

- Clap tests prove `chat --follow`, targeting, conflicts, and the absence of a
  `wavechat` subcommand.
- Existing chat resolution/post tests and thread replay/render tests stay green.
- CLI help and user docs expose one human thread command.
- `cargo fmt`, focused tests, and `cargo clippy -- -D warnings` pass.

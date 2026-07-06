# typed blocks: give the compile a block vocabulary (convention, with agency)

Targets main. Slice 3 of the Memory wave. Linear: "memory: typed blocks in
MEMORY.md" (`2ae8a684`).

## What to build

Teach the `export-memory` compile to organize `MEMORY.md` into **typed blocks** —
as a *convention*, not code. No parser, no `Block` type, no enforced schema. The
compile prompt suggests a starting vocabulary and hands the agent ownership of
the shape.

**Agency is the point.** The blocks are defaults, not a cage: the agent renames,
adds, drops, or merges blocks to fit its wave. We give a vocabulary so structure
is *predictable enough* for a reader without dictating it. This is the same
"the fold lives in the mind" principle — we don't parse the mind's output, we
seed how it thinks.

## The change (prompt only)

In `engine/builtins/ops/step/export-memory.md`, extend step 3 ("Compile the
memory") with block guidance, roughly:

> Organize the compiled memory into typed blocks — short `##` sections, each one
> kind of knowledge. Common blocks, and good defaults to start from:
> - **Decisions** — choices made and why (the reason is the durable part).
> - **Constraints** — what must hold; what breaks if violated.
> - **Glossary** — domain terms, names, coinages the next mind needs.
> - **Roster** — who/what is involved, if the wave has people or components.
>
> These are a starting vocabulary, not a schema. You own this file's shape: add a
> block the wave needs, drop one it doesn't, rename to fit. Keep each block tight
> — a bloated block is a signal to split or prune, not to keep growing.

Optionally mirror one line into the general wave-memory guidance in
`engine/prompt.rs` (the "How to update" block) so hand-edits follow the same
convention: "organize into typed `##` blocks; these are defaults you can adapt."

## Constraints

- **Convention only.** No parsing, no budgets-in-code, no typed adds. If a real
  capability (per-block budget/injection, routed adds) proves worth it later,
  that's a separate slice — don't pre-build the parser.
- **Recency injection is unaffected.** Recent adds still layer above the whole
  base; blocks are the base's internal shape, not something injection parses.

## Demo

Run `lf export-memory --wave <wave>` on a wave with some `lf memory log` history:
`MEMORY.md` comes back organized into `##` typed sections the agent chose —
Decisions/Constraints/etc. where they fit, adapted where they don't.

## Done when

```bash
cargo test -p loopflow          # builtins test: export-memory prompt mentions
                                # the block vocabulary + agency
cargo clippy -- -D warnings && cargo fmt --check
```

Checklist: `export-memory.md` step 3 carries the block guidance + the explicit
"you own the shape, these are defaults" agency line; `builtins.rs` test updated
to assert it; no new code paths, no parser.

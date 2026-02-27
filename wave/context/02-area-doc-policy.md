# 02: Area Doc Policy

**Finish line:** `-a src/api/` gathers ancestor and descendant `.md` files. No siblings, no uncles.

Codify and implement the area document gathering policy: ancestors plus descendants, not siblings.

## Current behavior

`-a src/api/handlers` walks **ancestors only**: `src/*.md`, `src/api/*.md`, `src/api/handlers/*.md`. Each level is non-recursive — only `.md` files directly in that directory. Descendants and siblings are invisible.

## Target behavior

Area defines a cone: context above, detail below, nothing sideways.

`-a src/api/` gathers:
- **Ancestors** (current): `.md` files in each directory from the area up to the repo root. `src/*.md`, then repo root `.md` files.
- **Descendants** (new): `.md` files recursively under the area path. `src/api/**/*.md` — handlers, routes, middleware, everything below.
- **Not siblings or uncles**: `src/web/*.md` is not gathered. Only the direct ancestor chain and the subtree rooted at the area.

### Why descendants matter

Sub-directory READMEs are the author's explanation of how that subtree works. Without them, agents working in an area are blind to structure below them.

## Changes

**`rust/loopflow/src/engine/prompt.rs`**:
- `gather_area_docs()`: after the ancestor walk, add a recursive descendant walk using `gather_md_files()` on the area directory itself.
- Dedup: the area directory's own `.md` files are already gathered in the ancestor walk — don't double-count.
- Tag descendants as `DocumentSource::Area` (same as ancestors).

**Budget implications**:
- Descendant docs are lowest priority within area docs — drop descendants before ancestors when over budget.
- In `trim_context_with_breakdown()`, area docs are already the first thing dropped. Within area docs, drop descendant docs (deepest first) before ancestor docs.

### Ordering in prompt

Ancestors first (broadest context → narrowest), then descendants (area root → deepest). This gives agents the high-level picture before the details.

## Constraints

- Large monorepos could have hundreds of `.md` files under an area. The budget system handles this — area docs are first to be dropped. But we should also cap the walk depth or file count as a safety valve.
- Don't change behavior when `-a` is not passed. Repo root doc gathering stays non-recursive.

## Done when

- `lf implement -a rust/` in this repo includes `rust/loopflow/src/**/*.md` if any exist
- Ancestor docs still appear (no regression)
- Sibling directories are not gathered
- `cargo test -p loopflow` passes
- Audit header shows increased area doc count reflecting descendants

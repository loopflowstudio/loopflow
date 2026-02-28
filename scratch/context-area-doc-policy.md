# Area Doc Policy: Ancestors + Descendants

**Finish line:** `-a src/api/` gathers ancestor and descendant `.md` files. No siblings, no uncles.

## Problem

`gather_area_docs()` walks ancestors only. An agent working `-a src/api/` sees `src/README.md` and `src/api/README.md` but is blind to `src/api/handlers/README.md` and everything below. Sub-directory READMEs are the author's explanation of how that subtree works — without them, agents miss critical architectural context.

## Approach

Extend `gather_area_docs()` in `rust/loopflow/src/engine/prompt.rs` with a recursive descendant walk after the existing ancestor walk. Descendants are appended after ancestors so `pop()` during trimming drops them first.

### Changes

**`gather_area_docs()` (prompt.rs:822-895)**

After the ancestor loop, add a recursive walk of subdirectories under the area path. Skip the area directory itself (its `.md` files are already in `seen` from the ancestor walk). Cap at 100 descendant files as a safety valve for monorepos.

```rust
// After ancestor walk, gather descendants from subdirectories of the area
let area_abs = repo_root.join(area_path);
if area_abs.is_dir() {
    let mut descendant_docs = Vec::new();
    gather_area_descendants(&area_abs, repo_root, &mut descendant_docs, &mut seen);
    // Sort by path depth (shallowest first) so pop() drops deepest first
    descendant_docs.sort_by_key(|d| d.path.matches('/').count());
    // Safety cap: take at most 100 descendants
    descendant_docs.truncate(100);
    docs.extend(descendant_docs);
}
```

**New helper: `gather_area_descendants()`**

Can't reuse `gather_md_files()` — it strips paths relative to `dir.parent()`, but we need repo_root-relative paths for dedup with the `seen` HashSet. Write a focused helper:

```rust
fn gather_area_descendants(
    dir: &Path,
    repo_root: &Path,
    docs: &mut Vec<Document>,
    seen: &mut HashSet<String>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut sorted: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    sorted.sort_by_key(|e| e.path());

    for entry in sorted {
        let path = entry.path();
        if path.is_dir() {
            // Recurse into subdirectories
            gather_area_descendants(&path, repo_root, docs, seen);
        } else if path.extension().map(|e| e == "md").unwrap_or(false) {
            let rel_path = path
                .strip_prefix(repo_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            if seen.contains(&rel_path) {
                continue;
            }
            if let Ok(content) = fs::read_to_string(&path) {
                seen.insert(rel_path.clone());
                docs.push(Document {
                    path: rel_path,
                    content,
                    source: DocumentSource::Area,
                });
            }
        }
    }
}
```

**Prompt label update (prompt.rs:1501-1504)**

Change the area docs description from "Architectural context from parent directories" to "Architectural context from ancestor and descendant directories" so agents understand the scope.

**Doc comment update (prompt.rs:822-829)**

Update the `gather_area_docs()` doc comment to document the new behavior: ancestors + descendants, not just ancestors.

### No other changes needed

- **Trimming already works.** `area_docs.pop()` removes from the end. Since descendants are appended after ancestors, they're dropped first. Within descendants, deepest-first sort means the deepest paths are at the end and get dropped first. This matches the design doc's priority: drop descendants before ancestors, deepest first.
- **`gather_documents()` call site unchanged.** It already calls `gather_area_docs()` and routes everything through `DocumentSource::Area`.
- **`ContextBreakdown` unchanged.** `area_doc_count` already counts all area docs.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Reuse `gather_md_files()` | Zero new code | Strips paths relative to `dir.parent()`, not repo_root. Would need to change its signature and audit the scratch docs caller. More risk for less clarity. |
| Add `base_path` param to `gather_md_files()` | One function for all recursive walks | Changes behavior for scratch doc gathering. The two use cases have different dedup needs (seen set vs. none). Coupling them is false economy. |
| Walk descendants in `gather_documents()` separately | Clearer separation | Would require passing `seen` between two functions or accepting duplicates. The ancestor and descendant walks share dedup state — they belong together. |
| No cap on descendants | Simpler | A monorepo area like `-a src/` could pull thousands of `.md` files. The budget trimmer handles it eventually, but counting tokens on 2000 files is slow. Cap at 100 is cheap insurance. |

## Key decisions

1. **New helper instead of reusing `gather_md_files()`.** The existing function uses `dir.parent()` for path stripping and has no dedup. Rather than adding parameters that complicate the scratch docs path, write a focused 25-line helper that does exactly what area descendants need.

2. **Cap at 100 descendant files.** Budget trimming handles overshoot, but it's expensive to tokenize hundreds of files just to drop most of them. 100 is generous enough for any reasonable area and cheap insurance against pathological cases.

3. **Sort descendants by depth, shallowest first.** This means `pop()` removes the deepest files first during trimming — exactly the right priority. Shallow files (closer to the area root) are more likely to contain high-level architectural context.

4. **Descendants skip the area directory's own files.** The `seen` HashSet from the ancestor walk ensures no double-counting. The descendant walk starts by recursing into subdirectories, not re-reading the area directory's direct `.md` files.

## Scope

- **In scope:** Recursive descendant gathering in `gather_area_docs()`, dedup, depth-first trimming, safety cap, doc comment and prompt label updates, tests.
- **Out of scope:** Changing `gather_md_files()` signature. Changing repo root doc behavior. Changing budget priorities between sources.

## Done when

- `cargo test -p loopflow` passes
- A test creates nested `.md` files under an area, runs `gather_area_docs()`, and verifies both ancestors and descendants are returned
- A test verifies sibling directories are excluded
- A test verifies the 100-file cap
- Audit header label reflects "ancestor and descendant" context
- `lf implement -a rust/` in this repo includes `rust/loopflow/src/**/*.md` if any exist

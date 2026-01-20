# Folders

Unified file storage conventions for loopflow: documented folder hierarchy, `.docs/` auto-inclusion, and goal loading for autonomous agents.

## Review

**Verdict:** Ready to ship

All checklist items complete. The implementation correctly:

1. **Auto-includes `.docs/`** — `gather_internal_docs()` added to `design.py`, integrated into context assembly in `context.py`. Order is `.design/` → `.docs/` → root docs.

2. **Keeps `docs/` opt-in** — Public docs not auto-included; must be added via `context:` in config.

3. **Goal loading works** — `load_goal()` in `design.py` handles both name lookup (`.lf/goals/{name}.md`) and explicit paths. Agent runner injects goal content with `<lf:goal:...>` tags.

4. **Built-in prompts updated** — `design.md`, `implement.md`, `review.md` now reference `.docs/` in their workflows.

5. **Documentation complete** — `docs/storage.md` explains philosophy and folder conventions. Quick reference table updated to show `.docs/` as auto-included.

6. **Tests added** — `test_context.py` covers `.docs/` inclusion, order, and `docs/` exclusion. `test_design.py` covers `gather_internal_docs()` and `load_goal()`. 29 summarize tests added.

Minor style note: the inline imports in `summarize.py` (lines 245, 277, 312, 327) are intentional to avoid circular imports at module load time. The pattern is acceptable here.

## Design notes

**Ephemeral vs persistent distinction.** `.design/` is cleared on merge; `.docs/` persists. This enforces the rule: if it matters after merge, put it in `.docs/`.

**Context order.** The prompt assembles docs in order: `.design/` (ephemeral, most specific to current work), `.docs/` (persistent internal), then root `.md` files. This puts the most relevant context first.

**Goal injection.** Goals are wrapped in `<lf:goal:{name}>` tags when injected, making them visually distinct from task prompts in the assembled context.

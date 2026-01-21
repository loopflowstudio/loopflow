---
choice: add_to_roadmap
reason: No roadmap exists yet; must populate .docs/roadmap before scoping work
options: [add_to_roadmap, scope_from_roadmap]
---

The `.docs/roadmap` directory does not exist and `.design` is empty. The `scope_from_roadmap` branch requires an existing roadmap to select work from, but there's nothing to scope.

Running `add_to_roadmap` will create initial roadmap items by forking to three models (claude, gemini, codex) with different voice combinations, then joining the results.

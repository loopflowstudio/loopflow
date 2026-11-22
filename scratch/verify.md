# verify dd8997 — 2026-11-21

PR #2012 merged as 891d9a9b92; `git diff 9cf44fc2dc 891d9a9b92` is empty, so main's tree is the reviewed head.

- Post-merge CI on 891d9a9b: `macOS on main` success; push `CI` in_progress at check time (merge_group CI success). Push run unread — CI owns it.
- `website/tests`: `uv run --no-sync python ../scripts/test_network.py uv run --no-sync pytest tests/ -q` (LF_* unset, stdin closed) → 99 passed.
- style.css on main: 0 `!important`, 0 `--d-*`, no hex outside `:root`; Google Fonts appear only in test_faces.py's forbidden-host list.
- Workflow card (website/home.html:56-58) matches builtin feature/code/research graphs step for step.
- Steer items: 404 handler renders a site page (main.py:29 `not_found_page`); Pico blue focus ring cleared (style.css:163).
- Not re-checked here: visual pass at 1440/390 light/dark — done in ship-review/QA before merge.

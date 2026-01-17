# newux

Adds `--prompt`/`-p` CLI option to chain prompt files, plus four UX improvement prompts for Maestro research and iteration.

## Review

**Verdict:** Ready to ship

Clean implementation. Two commits: design doc, then the feature. The `--prompt` flag works as specified—appends additional `.lf/*.lf` content to the task prompt with `---` separator. Case-insensitive short flags (`-p`/`-P`, `-a`/`-A`, etc.) added consistently across `run`, `inline`, `cp`, and `pipeline` commands.

The UX prompts (nux, ux-research, ux-gaps, ux-fix) are well-structured with clear outputs and appropriate constraints. The pipeline config ties them together correctly.

## Design notes

**Prompt chaining**: Multiple `-p` flags accumulate. Content appends in order with `---` separator. If a prompt file isn't found, it's silently skipped (no error)—worth noting if someone typos a prompt name.

**Context inheritance**: The `with_prompts` parameter passes through `gather_prompt_components` → `gather_task`. Frontmatter from chained prompts is ignored; only their content is appended. This is the right call—frontmatter config should come from the main task.

**UX workflow dependency**: ux-research and ux-gaps require screenshots in `.design/screenshots/`. The prompts note this, but first-time users won't have screenshots. Consider adding a check or clearer guidance in ux-research.lf about capturing screenshots first.

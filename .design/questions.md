# Open Questions

## Resolved

- **macOS version:** 15+ (Sequoia). Users are sophisticated.
- **Worktree management:** Full CRUD. No safer than CLI.
- **Distribution:** Direct (.dmg / Homebrew), not App Store.
- **Daemon:** App reads SQLite directly. CLI writes to `~/.lf/maestro.db`, app polls it. No daemon needed for tracking—agents are just processes that persist independently.
- **Double-click:** Opens in terminal. (What would Notion do? Open the thing.)
- **Context picker:** Notion-style tree view with toggles.
- **Token estimation:** Shell out to `lf -c` for accuracy.
- **Default prompt:** None. We don't know which prompts exist in any given repo.
- **Config keys:** `terminal: warp`, `ide: cursor` (separate top-level keys, not nested).
- **App name:** "Loopflow Maestro". Will rethink `lf ops maestro` CLI command.

## Remaining

### Branding

1. **Logo:** The loopflow.studio site is a different company (music). Need logo source or design fresh.

### CLI changes needed

2. **Config migration:** Update Python config to use `terminal:` and `ide:` as top-level keys instead of nested `ide.warp`, `ide.cursor`.

3. **Rename `lf ops maestro`:** Since the Mac app is "Maestro", the CLI daemon command should be renamed. Options:
   - `lf ops server` (generic)
   - `lf ops daemon` (descriptive)
   - Or remove it entirely if the Mac app replaces it

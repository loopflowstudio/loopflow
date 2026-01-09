# Review Guide: post-winter

## What This Branch Does

This branch is a major iteration on loopflow's architecture and feature set, implementing several interconnected improvements:

1. **Worktrunk-compatible worktree layout** - Moves from `.lf/worktrees/{name}` to sibling directories `../{repo}.{name}`, compatible with worktrunk tooling
2. **Multiple backend support** - Abstracts LLM backends (Claude, Codex) behind a common interface
3. **Enhanced worktree visualization** - Rich `lf wt list` output with CI status, PR links, diff stats, and dependency trees
4. **Task argument substitution** - Template variables in task files (e.g., `{{name_a}}`) for parameterized prompts
5. **Improved `lf meta init`** - Bundles prompts, style guide, and config template in the package for easy repo setup
6. **Design doc lifecycle** - `lf review` now transforms design docs into review guides; `lf pr land` removes them
7. **Compare command** - `lf wt compare <a> <b>` for analyzing different implementations

## Key Design Decisions

### Worktrunk Compatibility

The switch to sibling directories (`../{repo}.{name}`) enables interop with worktrunk and avoids `.lf/worktrees/` clutter. This is a **breaking change** - existing worktrees in the old location won't be found.

**Tradeoffs**: Users with existing worktrees will need to recreate them. But the new pattern is cleaner and more standard.

### Backend Abstraction

The `Backend` ABC in `launcher.py` provides a clean interface for multiple LLM backends. Currently supports Claude Code and a placeholder Codex implementation.

**Risk**: The Codex backend hasn't been tested against a real Codex CLI (it may not exist as implemented). This is speculative code.

### Worktree List Complexity

`list_worktrees()` in `git.py` now performs many git/gh operations per worktree (commit info, PR status, CI checks, diff stats). This could be slow with many worktrees.

**Tradeoffs**: Rich information vs. performance. The JSON output mode provides an escape hatch for scripting.

### Template Variable Substitution

Task args use simple string replacement (`{{key}}` → value). No escaping or quoting - values are inserted verbatim.

**Risk**: If values contain `{{` or `}}`, results may be unexpected. This is simple but fragile.

## Review Checklist

- [ ] **Breaking change**: Verify migration path for users with existing `.lf/worktrees/` worktrees. Should this be documented in release notes?
- [ ] **Codex backend**: Is this a real thing? If not, should it be removed or marked as experimental?
- [ ] **Performance**: Test `lf wt list` with 10+ worktrees. Does it feel responsive?
- [ ] **Design doc removal**: Verify `lf pr land` correctly removes `<branch>.md` and stages the deletion
- [ ] **CI integration**: Test with a real GitHub repo that has PR checks. Do the ✓/✗/● indicators work?
- [ ] **Template variables**: Try edge cases like nested `{{var}}` or values containing template syntax
- [ ] **Bundled assets**: Verify `lf meta init` works in a fresh repo (not just loopflow itself)

## What's Unfinished

The `.lf/wt-visualization.md` design doc suggests this feature is complete, but the worktree migration story needs clarity. Users upgrading from older versions will lose visibility of their old worktrees.

## Notable Files Changed

- `src/loopflow/git.py:58-290` - Worktrunk-compatible paths and rich worktree info gathering
- `src/loopflow/cli/wt.py:7-457` - Completely rewritten `lf wt list` with table formatting
- `src/loopflow/launcher.py:3-107` - Backend abstraction with Claude and Codex implementations
- `src/loopflow/cli/pr.py:278-283` - Design doc removal in `lf pr land`
- `src/loopflow/cli/meta.py:40-107` - `lf meta init` with bundled templates
- `src/loopflow/context.py:46-149` - Task argument substitution via template replacement

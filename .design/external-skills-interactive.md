# External Skills Interactive Mode

External skills (like `sp:brainstorm`) now default to interactive mode.

## Changes

### Default to interactive mode for external skills

External skills are third-party prompts designed for exploration and conversation. They should default to interactive mode, not auto mode. The config resolution order is now:

1. CLI (`-i` or `-a`)
2. Frontmatter (`interactive: true/false`)
3. Global config (`interactive: [step-list]`)
4. **External skill default** (interactive if `is_external_skill`)
5. Default (auto)

### Auto-detect rams.ai as single-file skill source

When `~/.claude/commands/rams.md` exists, it's auto-detected as a skill source with prefix `rams`. This allows running `lf rams:rams` to invoke the rams accessibility review skill.

Single-file skills are a new skill source kind where the skill name matches the filename directly (rather than being a directory with `SKILL.md`).

### Exclude single-file skills from global steps list

Single-file skills like rams that live in `~/.claude/commands/` are now properly excluded from the global steps list when they're handled as external skills. This prevents duplicate entries in `lf --list`.

## Testing

- `test_resolve_step_config_external_skill_defaults_interactive` - verifies external skills default to interactive
- `test_resolve_step_config_external_skill_cli_auto_overrides` - verifies `-a` flag overrides
- `test_resolve_step_config_external_skill_frontmatter_overrides` - verifies frontmatter overrides
- `test_discover_skill_sources_auto_detects_rams` - verifies rams auto-detection
- `test_find_skill_handles_single_file_source` - verifies single-file skill loading

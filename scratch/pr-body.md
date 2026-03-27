## Try it!
- `target/debug/lf --list | sed -n '/gstack/,+31p'`
  - See the imported `gstack:*` workstyle steps in the normal step listing.
- `target/debug/lf-prompt --repo . --step gstack:office-hours --surface headless --lfdocs false --diff false --diff-files false | rg 'gstack:(ceo-review|eng-review|design-review)'`
  - Verify the imported prompt now references loopflow-native follow-on steps instead of old `/plan-*` commands.
- `uv run pytest python/tests/test_workstyle_convert.py -v`
  - Covers converter output, direction extraction, and the new reference-rewrite behavior.

## Intent
Stage 1 already imported gstack into a loopflow workstyle, but several converted prompts still talked like they were running inside gstack: old `/plan-*` step names, raw `~/.claude/skills/gstack/.../SKILL.md` paths, and leftover retro analytics instructions. This pass makes the shipped artifact match the actual loopflow surface by rewriting those references during conversion and updating the committed workstyle bundle to match.

## Assumptions
- Stage 1 continues to import generated `SKILL.md` files, not gstack templates.
- It is acceptable in this stage for some deeper gstack utility references (`gstack-review-read`, `gstack-config`, etc.) to remain as future integration debt rather than blocking the initial workstyle import.
- The committed `.lf/workstyles/gstack/steps/*.md` files are expected to stay in sync with converter behavior.

## Key decisions
- Rewrite cross-step references to `gstack:<step>` names instead of preserving slash-command text.
- Rewrite imported skill-file paths to `.lf/workstyles/gstack/steps/*.md` so inline read instructions point at files that actually exist in loopflow.
- Strip retro analytics/eureka instructions during conversion because they are telemetry scaffolding, not reusable methodology.

## Not included
- No loopflow-native implementation for gstack helper binaries or review log storage.
- No automated re-sync command for re-importing gstack.
- No stage-2/stage-3 flow or sync work.

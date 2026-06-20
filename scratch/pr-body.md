## Try it!

```bash
lf op sync-skills
find .claude/skills .agents/skills -name SKILL.md | head

lf design --tui
lf gate --ide

uv run python scripts/verify_skill_sync.py
```

`lf op sync-skills` writes repo-local generated skills. The interactive commands should open a vendor session with a small `/step` seed instead of a giant assembled prompt.

## Intent

Move step handoffs off the launch prompt channel and onto vendor Skills. Claude and Codex read the synced `SKILL.md` bodies on explicit `/step` invocation, so terminal and app handoffs carry only the surface preamble, voice, orientation, and any user message.

## Assumptions

- Claude reads `.claude/skills/<step>/SKILL.md`; Codex reads `.agents/skills/<step>/SKILL.md`.
- Explicit slash invocation is the execution path; model auto-invocation by description is out of scope.
- Global skill writes should remain opt-in because they touch `~/` vendor directories.

## Key decisions

- Generate skills with a `loopflow: true` marker and prune only marked stale skills.
- Default to repo-local skill sync on launch; require `--global --yes` or confirmation for global sync.
- Keep voice and branch orientation in the launch seed rather than mutating human-authored `AGENTS.md` / `CLAUDE.md` symlinks.
- Keep `npx/*` and `rams/*` on the assembled-prompt fallback until external skill namespace rules are designed.
- Pin `rusqlite` to `0.39` so stable CI avoids the unstable `cfg_select` path in `libsqlite3-sys 0.38.x`.

## Not included

- Direction removal and DTO migration.
- Flow-as-skill conversion.
- Skill auto-invocation by description.
- Live vendor probe in gate; use `uv run python scripts/verify_skill_sync.py --live` when Claude and Codex CLIs are authenticated.

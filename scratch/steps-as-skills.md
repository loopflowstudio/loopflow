# Steps as Skills — validation

The steps-as-skills milestone shipped on this branch: loopflow stops assembling
prompts for handoffs. Steps are vendor **Skills** on disk; the launch seed is a
surface preamble plus a harness-aware skill invocation (`/step` for Claude,
`$step` for Codex). Both headless and interactive runs use the same path. Design
rationale lives in `release/unreleased/DECISIONS.md` (2026-06-19, 2026-06-24).

The deferred follow-on (removing the `direction` wire field) is tracked in
`wave/workflows/3-remove-directions.md`, not here.

## Try it

```bash
lf op sync-skills
find .claude/skills .agents/skills -name SKILL.md | head

lf design --tui     # Claude TUI: seed is "<surface> /design"
lf gate --ide       # vendor GUI: deep link pre-fills the skill seed
```

`sync-skills` writes repo-local generated skills (marked `loopflow: true`) into
`.claude/skills` and `.agents/skills`; re-sync prunes stale generated skills
without touching user-owned ones. Add `--global --yes` to also write under `~/`.

## Done when (verification)

- `lf op sync-skills` writes every resolved step as a `SKILL.md` to the four
  targets; Claude emits carry `disable-model-invocation: true`; generated skills
  are marked and prunable.
- `lf <step>` (interactive or headless) syncs skills, clears the assembled system
  prompt, and seeds the harness-specific skill invocation; the vendor session
  runs the step body from its synced skill — no ~100KB assembled prompt.
- Loopflow's operating manual lives in this repo's agent doc (`STYLE.md` via the
  `CLAUDE.md` / `AGENTS.md` symlinks); branch orientation lives in step bodies.

```bash
cargo test -p loopflow launch_prompt golden_prompt context_tests skill
cargo clippy --all-targets -- -D warnings
uv run python scripts/verify_skill_sync.py          # structural probe
uv run python scripts/verify_skill_sync.py --live    # sync + fire under claude -p / codex exec
```

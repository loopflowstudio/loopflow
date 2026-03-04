# v0.9.7

Loopflow 0.9.7 strips ~1600 lines of ops scaffolding, signs the macOS installer, and moves agents closer to self-directed execution — they assemble their own context, discover their own checks, and author their own PRs.

## Simplification

- **Agents own their context** — deleted `ops/messages.rs`, `ops/lint.rs`, `ops/agent.rs`, and `ops/combine.rs` (~1400 lines). Agents now assemble context directly through steps instead of the engine pre-building it. Release nests under five focused subcommands: `lf ops release check`, `notes`, `bump`, `tag`, `status`
- **Agents own their checks** — removed `lf ops lint` and `lf ops test` commands along with the `lint:` and `test:` config fields. Agents discover what to run from `TESTING.md` and CI config, which is where the information already lived
- **Simpler PR creation** — `lf ops pr` now requires `--title` and `--body` (no more `Option`). Removed the `--refresh` flag and its origin-before/after diff-detection logic. The `lf pr` step handles generation
- **Prompt handoff renamed** — `.lf/log/` → `.lf/prompts/` for clarity on what it holds (agent-readable prompt files). Diagnostic output moves to `~/.lf/logs/<repo>/<worktree>/` outside the repo, preventing accidental commits

## Infrastructure

- **Signed macOS installer** — `concerto-dev.py release` codesigns the .app with Developer ID, signs the DMG, and submits for Apple notarization. CI imports the signing certificate from Doppler and cleans up the keychain afterward. R2 credentials also moved from GitHub Secrets to Doppler
- **Sibling worktree enforcement** — `wave_name_from_worktree_and_main` now requires worktrees to share the same parent directory as the main repo. Worktrees created inside the repo (e.g., `.claude/worktrees/`) return `None` instead of incorrectly producing a wave name
- **Land step rewritten** — single `lf ops land` call replaces the manual git/gh sequence. Headless surface detection added for non-interactive sessions

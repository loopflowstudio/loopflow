# Review: worktree-loopflow.dmg

Four commits across ops cleanup, DMG signing, PR simplification, and log relocation.

## What was implemented

1. **Codesigning and notarization** — `scripts/concerto-dev.py` now detects Developer ID certificates, codesigns the .app bundle with hardened runtime + entitlements, signs and notarizes the DMG, and staples the ticket. CI workflow (`release.yml`) fetches secrets from Doppler, imports the signing certificate into a temporary keychain, and cleans it up afterward.

2. **`lf ops pr` made strict** — `--title` and `--body` are now required `String` args (not `Option<String>`). The `--refresh` flag and its "no title needed" codepath are removed. `PrOptions` fields simplified from `Option<String>` to `String`.

3. **`lf ops lint` and `lf ops test` removed** — The `lint:` and `test:` config fields are deleted from `Config`, along with the `run_check()` function and the CLI subcommands. Gate and lint steps now direct agents to read `TESTING.md` and CI config directly instead of relying on configured commands.

4. **Prompt logs renamed and durably copied** — `.lf/log/` → `.lf/prompts/` for in-repo prompt files. A durable copy is now written to `~/.lf/logs/<repo>/<worktree>/` so prompts survive worktree deletion.

5. **Sibling worktree enforcement** — `wave_name_from_worktree_and_main` now rejects non-sibling worktrees (e.g., `.claude/worktrees/`). Added test. LOOPFLOW.md documents the sibling convention and warns against agent-provided worktree tools.

6. **Surface detection** — `lf` CLI now sets `Surface::Headless` for non-interactive (piped) invocations instead of always using `Surface::Cli`.

7. **Secrets migration** — R2 upload credentials moved from GitHub Actions secrets to Doppler. `_load_r2_credentials()` prefers Doppler, falls back to env.

## Key choices

- **Doppler for secrets** — centralizes secrets management across CI and local dev. GitHub secrets env vars removed from workflow YAML; Doppler action injects them instead.
- **Required title/body on `lf ops pr`** — forces agents to generate meaningful PR content via the `lf pr` step rather than allowing empty refresh-only operations. Simplifies `pr.rs` significantly (~60 lines removed).
- **Removing `lint:`/`test:` config** — agents reading `TESTING.md` is more reliable than a single configured command string. Agents can discover and run the right subset of checks for their changes.
- **Sibling enforcement via canonicalize** — uses `canonicalize()` to handle macOS `/tmp` → `/private/tmp` symlinks. Returns `None` (not an error) for non-sibling worktrees so callers degrade gracefully.

## How it fits together

`lf ops pr` and `lf ops land` share `PrOptions` — both now require title+body. The land step's `ensure_pr` validates these are present when `--create-pr` is set. The PR step (`pr.md`) documents the new requirement and drops the `--refresh` notes.

The prompt log rename is threaded through: `write_prompt_log` → `.lf/prompts/`, `durable_log_dir` → `~/.lf/logs/`, `.gitignore` entries, `LOOPFLOW.md` docs, test sandbox scripts, wave executor test helper, and all 7 golden files.

## Risks and bottlenecks

- **Doppler availability in CI** — if `DOPPLER_TOKEN_PRD` secret is missing or Doppler is down, the release workflow fails. No fallback to GitHub secrets.
- **Notarization latency** — `--wait` on `notarytool submit` blocks CI. Apple notarization typically takes 5-15 minutes.
- **`canonicalize()` in worktree check** — requires the parent directories to exist on disk. Non-existent paths return `None` (safe, but could mask issues in tests with virtual paths).

## What's not included

- No migration for existing `.lf/log/` directories — old prompt logs stay where they are.
- No Doppler fallback if the service is unreachable in CI.
- GhosttyKit binary target URL/checksum update is mechanical (new build hash), not behavioral.

# v0.9.8

Loopflow 0.9.8 makes flows mechanical where they should be, encrypts secrets at rest, and recovers from rebase conflicts without stopping. Agents handle judgment; `ops:` items handle plumbing.

Changes since `v0.9.7`.

## Flows own their plumbing

- **`ops:` is a first-class flow item** — flows can now execute `land`, `rebase`, and `release` directly via Rust functions instead of spinning up an agent session. Gate writes PR copy to `scratch/`; land reads it, validates freshness, and clears it after use
- **`lf ops land` auto-generates PR copy** — title and body are produced via Claude API from the diff when not provided explicitly. No more required `--title`/`--body` flags
- **One-shot release** — `lf ops release run patch` owns the full lifecycle: check PRs, bump manifests, generate notes, create and land a release PR, tag the merged commit, and wait for the release workflow. CI release is re-runnable: auto-tag dispatches the release workflow explicitly, and publish steps tolerate already-published versions

## Ops recover from conflicts

- **Rebase conflict auto-recovery** — `lf ops rebase`, `land`, and `pr` now launch a step agent to resolve rebase conflicts instead of failing. The agent gets structured conflict context and fixes things up, then the original command retries
- **`land` stages uncommitted changes** — no more forgetting to `git add` before landing
- **Worktree pruning simplified** — merged worktrees prune without confirmation; `--force` replaced by `--include-fresh` for worktrees with no commits beyond main

## Trust

- **Tokens encrypted at rest** — provider tokens in SQLite/Postgres are now AES-256-GCM encrypted with keys stored in the OS keychain (macOS Keychain, Linux secret-tool, file fallback). Existing plaintext tokens migrate automatically on startup
- **Secret redaction** — sensitive values are masked in logs and agent output

## Context visibility

- **Session context UI in Concerto** — expandable panel showing which files, area docs, and diff chunks are loaded into an agent's prompt, with per-document token counts and trimmed/included status. `ContextBreakdown` now tracks structured per-document metadata through the HTTP API

## Cleanup

- **~13,000 lines of stale artifacts deleted** — `.agents/skills/`, `proto/`, `reports/`, and `bin/` scripts removed. Unused config fields (`push`, `include_loopflow_doc`) stripped from the `Config` struct
- **`lf ops lint` and `lf ops test` removed** — agents discover lint/test commands from `TESTING.md` and CI config directly. The `lint:` and `test:` config fields are gone
- **Init separates repo and user config** — `lf init` now distinguishes repo config (agent, harnesses, exclude) from user config (yolo, ide, chrome) and offers to create `~/.lf/config.yaml` when missing
- **DMG codesigning pipeline** — `concerto-dev.py release` codesigns the .app with Developer ID, signs the DMG, and submits for Apple notarization. Resource bundles moved into `Contents/Resources/` to fix unsealed-contents errors
- **Worktree rotation improved** — creation syncs the default branch from origin before branching; archived worktrees use the branch's own timestamp; squash-merge detection tightened to avoid false positives; sibling directory layout enforced
- **`git2` crate removed** — replaced by git CLI calls

# v0.9.8

Loopflow 0.9.8 makes flows mechanical where they should be, encrypts secrets at rest, and deletes ~13,000 lines of stale artifacts. Agents and ops split the work cleanly — agents handle judgment, `ops:` items handle plumbing.

Changes since `v0.9.7`.

## Flows own their plumbing

- **`ops:` is a first-class flow item** — flows can now execute `land`, `rebase`, and `release` directly via Rust functions instead of spinning up an agent session. Gate writes PR copy to `scratch/`; land reads it, validates freshness, and clears it after use
- **`lf ops land` auto-generates PR copy** — title and body are produced via Claude API from the diff when not provided explicitly. No more required `--title`/`--body` flags
- **One-shot release** — `lf ops release run patch` owns the full lifecycle: check PRs, bump manifests, generate notes, create and land a release PR, tag the merged commit, and wait for the release workflow

## Trust

- **Tokens encrypted at rest** — provider tokens in SQLite/Postgres are now AES-256-GCM encrypted with keys stored in the OS keychain (macOS Keychain, Linux secret-tool, file fallback). Existing plaintext tokens migrate automatically on startup
- **Secret redaction** — sensitive values are masked in logs and agent output

## Context visibility

- **Session context UI in Concerto** — expandable panel showing which files, area docs, and diff chunks are loaded into an agent's prompt, with per-document token counts and trimmed/included status. `ContextBreakdown` now tracks structured per-document metadata through the HTTP API

## Cleanup

- **~13,000 lines of stale artifacts deleted** — `.agents/skills/`, `proto/`, `reports/`, and `bin/` scripts removed. Unused config fields (`push`, `include_loopflow_doc`) stripped from the `Config` struct
- **Init separates repo and user config** — `lf init` now distinguishes repo config (agent, harnesses, exclude) from user config (yolo, ide, chrome) and offers to create `~/.lf/config.yaml` when missing
- **Worktree rotation improved** — creation syncs the default branch from origin before branching; archived worktrees use the branch's own timestamp instead of current time; squash-merge detection tightened to avoid false positives

# Gate Review: Cross-Platform Init & Ingest Fast-Path

Branch: `jack-heart.init.20260228_0831`
Wave: `docs` (sprint 01-setup)

## What was implemented

Two deliverables from `wave/docs/01-setup.md`:

**1. Init prompt rewrite** — Complete rewrite of `init.md` (both `.lf/steps/` and builtin copies). The old init hard-gated on macOS, required Homebrew, tried to install things via `brew`/`npm`, referenced Gemini CLI (no harness), and hardcoded `claude:opus`. The new init:

- Detects platform and installed agents with `command -v` (POSIX) instead of `which`
- Never installs — only detects. Shows install instructions if nothing is found
- Supports Claude Code, Codex, and OpenCode (Gemini removed — no harness)
- Defaults to agent name without model suffix (`claude` not `claude:opus`)
- Creates `.lf/config.yaml` and nothing else (no flows README, no IDE config)
- Platform-aware next-steps (Concerto on macOS, tmux on Linux)
- Re-run safe: compares existing config against detected agents, offers to update

**2. Ingest fast-path** — New `lf ops ingest` CLI command that deterministically picks the lowest-numbered wave item and moves it to `scratch/`. Wired into the `ingest` step via `fast-path: lf ops ingest` frontmatter. Exit 0 skips the LLM agent; non-zero falls through.

**3. Headless surface rewrite** — More directive language: "Make executive decisions and keep moving" instead of "make best-effort assumptions." Propagated to LOOPFLOW.md, STYLE.md, AGENTS.md, and all golden test files.

**4. Wave plan created** — `wave/docs/` with README, 3 sprint files, and `docs.yaml` config.

**5. Design updates** — `design.md` and `update-wave.md` now require creating every sprint file (even sketches) so `ingest` has files to pick.

## Key choices

| Decision | Why | Alternative rejected |
|----------|-----|---------------------|
| Detection-only init | Safe on any platform, can't break anything | Keep installer approach — fragile across distros |
| Remove Gemini CLI | No harness exists yet | Keep references — creates false expectations |
| `command -v` over `which` | POSIX builtin, consistent behavior | `which` varies across distros, may not exist |
| Agent name without model (`claude` not `claude:opus`) | Engine handles model defaults in `parse_agent()` | Hardcode model — couples init to model availability |
| Ingest as ops command | Deterministic; avoids LLM for simple file operations | Keep LLM-only — slow and unpredictable for common case |
| Fast-path via frontmatter | Step declares its own fast path; engine handles fallback | Hardcode in engine — couples engine to step knowledge |

## How it fits together

The init rewrite is a self-contained prompt change — no Rust code changes needed. The engine already renders init.md as a step; the prompt content is what changed.

The ingest fast-path adds `ops/ingest.rs` (new module) → `OpsCommand::Ingest` variant → `lf ops ingest` CLI subcommand. The `ingest` step's frontmatter declares `fast-path: lf ops ingest`. When the step runner encounters this frontmatter, it runs the command first; if exit 0, it skips the LLM agent entirely.

Headless surface changes are pure content — propagated to LOOPFLOW.md (included in all prompts), the surface file, STYLE.md, AGENTS.md, and golden tests.

## Risks and bottlenecks

- **Init is still a prompt.** Cross-platform detection logic in a markdown prompt relies on the LLM correctly interpreting and running the specified bash commands. If an agent misinterprets the instructions (e.g., still tries to install), the user gets a bad experience. Mitigated by keeping init focused on `command -v` and `test` — hard to misinterpret.

- **Ingest file move is not atomic.** `std::fs::copy` + `std::fs::remove_file` could leave the file in both places if the process is killed between the two operations. Low risk in practice — the file is small and the operation is fast.

- **Terminology divergence.** `.lf/steps/ingest.md` uses "sprints" while `builtins/steps/plan/ingest.md` uses "stages." Pre-existing, not introduced by this branch, but noted in the wave README risks.

## What's not included

- Concerto SetupView/SetupService changes (wave scope: "not Concerto features")
- Gemini CLI support (wave scope: "not here")
- `lfd install` changes (already cross-platform)
- Docs restructure (sprint 02)
- Reference accuracy pass (sprint 03)

## Gate polish applied

- Removed stale Gemini CLI references from README.md (intro line + Integrations section) — was inconsistent with Requirements section that already dropped it
- Verified init.md copies are identical between `.lf/steps/` and builtin
- All 693 Rust tests pass, golden prompt tests pass, clippy clean, fmt clean

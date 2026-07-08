# v0.9.11

Loopflow 0.9.11 picks a side: it is the orchestration layer above the agent vendors, not a chat client. The native session UI and the mobile-as-an-app surface come out; interactive work hands off to Claude Code, Codex, and opencode in their own surfaces. Skills become vendor Skills instead of a 100KB assembled prompt. Release automation goes self-hosted end to end, and the desktop app ships as **Loopflow**.

Changes since `v0.9.10`.

## Loopflow is the layer above

The recurring temptation was to reimplement the vendors' own session UIs — a native chat client, a mobile pairing flow, an lfd layer that parsed Claude/Codex streams into our event model. That work doesn't compound: the vendors ship better chat, IDE, and mobile clients than we will, and faster. So Loopflow skills back to the orchestration layer and runs headless agent work; when a human drives a session, it happens in the vendor's surface.

- **Sessions launch in the vendor's app** — a plain `lf <skill>` opens a fresh session directly in Claude Code or Codex when configured that way. Terminal-first; Concerto is just another caller
- **Embedded terminal stays** — a tmux pane running the vendor's own TUI inside Concerto. Concerto frames it; the vendor renders it
- **Bounce to the vendor's IDE** — open the worktree in VS Code / Cursor / JetBrains where the vendor's extension runs the session
- **Native chat UI and the session-hosting harness removed** — the SwiftUI chat client and the ~7,800-line `lfd/sessions/harness` stream-parser are gone; they existed only to feed a UI we're no longer building
- **Mobile-as-an-app archived** — the iOS target, `lf op pair`, pairing tokens, and remote-lfd-for-phone infra are dropped. Mobile happens through the vendors' own mobile apps. Concerto stays the macOS surface, raised a layer: wave monitoring plus the frame around vendor TUIs

## Skills become vendor Skills

Every path to "inject our context at launch" was blocked at once — argv truncation, Claude's competitor-mention guard on system prompts, and ~5KB caps on GUI deep links. Both vendors had already shipped the same answer for reusable instructions: **Skills** (the open `SKILL.md` standard, loaded progressively). So loopflow stops assembling a prompt for handoffs. The execution model becomes files on disk plus a tiny seed.

- **A sync emits each skill as a `SKILL.md`** into `.claude/skills/` and `.agents/skills/` at repo and global scope. Bodies stay out of context until the skill fires; generated skills carry a provenance marker so re-sync prunes safely
- **The seed is just `"<surface preamble> /skill"`** — the only per-run injection. Identical headless and interactive, save the run-mode preamble. Verified to fire under headless `claude -p` and `codex exec`, not only interactively
- **Harness-aware sigil** — `$skill` for Codex (works in both `exec` and the interactive composer), `/skill` for Claude
- **Ambient context moves to disk** — repo conventions, voice, and orientation live in `AGENTS.md` / `CLAUDE.md`, auto-loaded by the vendor. The agent reads `scratch/` and `wave/` on demand; we point, we don't dump
- **Directions removed as a first-class concept** — with no assembled prompt, a direction had no delivery vehicle. The `-d/--direction` flag, config field, wave key, loader, and `builtins/directions/` are gone; the perspective text was redistributed into the relevant skill bodies. The wave model simplifies from area × direction × flow to **area × flow**
- **`LOOPFLOW.md` leaves the product** — the operating manual is no longer injected into every session. It now lives in loopflow's own agent doc, loaded only when working on loopflow itself

## Self-hosted release automation

Release and cron automation had been drifting toward a private studio-hosted shape, with infrastructure living outside the repo. 0.9.11 reverses that: the release server is inspectable, reproducible, and maintainable from loopflow itself.

- **Self-hosted by default** — the public repo carries the runnable container and deployment shape; Doppler supplies secrets. Studio discovery is removed, not assumed
- **Studio auth deleted** — daemon registration and hosted discovery are gone. Remote `lfd` access is self-hosted bearer-token auth only; each repo owns its deploy config
- **Nightly verification, weekly release** — nightly package checks prove artifacts without deploying (`0 9 * * *` UTC); weekly publishing is gated by that verification (`0 12 * * 0` UTC). Loopflow and Cadenza run carbon-copy schedules
- **Native launchd `lfd` is the default Mac host path** — Docker Desktop's launchd/PATH/credential-helper friction made the container stack a poor first skill on macOS. `deploy/native-lfd-host.sh` centers the Mac runbook; Compose stays an explicit option for Linux and isolation needs
- **Cron host bootstrap, scheduled host updates, and native service env** — `lfd` persists its launchd environment, schedules its own updates, and keeps native tokens out of plists
- **Monthly spend guardrails** — stdlib-only cost tracking; automation spend over $100/month is the gate that needs a human
- **`wave/release/`** — owns daily verification, weekly publishing, cron-host infra, local-updater freshness, and cross-repo release parity

## The desktop app is Loopflow

The macOS app shipped as "Loopflow Concerto," but the product users actually use is Loopflow; Concerto was always the internal nickname.

- **Every user-facing surface renamed** — app bundle (`Loopflow.app`), display name, DMG volume and download keys (`Loopflow-<version>.dmg`, `Loopflow-latest.dmg`), and in-app titles
- **Concerto stays the dev nickname** — the Swift target, the bundle id `com.loopflow.concerto` (preserving granted TCC permissions and deep-link registration), and the debug `Concerto Dev.app` are unchanged
- **Version stamped from the release tag** — the app now reports the same version as `lf --version` instead of a frozen `1.0`

## One local build per worktree

Three overlapping install paths all wrote to global locations, so sibling worktrees fought over the same `lf` and the same `/Applications` app.

- **`scripts/install.py local` is the single entry** — builds this worktree's `lf`, `lfd`, and `Loopflow.app` into a gitignored `<worktree>/local-bin/`, isolated per worktree
- **`--use` promotes a build** — symlinks `~/.local/bin/{lf,lfd}` to the worktree and copies its `Loopflow.app` to `/Applications`. Symlinked binaries mean rebuilds take effect with no re-promote; switching active worktrees is one `install.py local --use`
- **The desktop app is a first-class release artifact** alongside `lf`/`lfd`. `pull-local-bin.sh` stays the CLI-only quick path and now ignores pull config

## Concerto navigation

- **Deep-link and menu navigation** — open a repo or portfolio directly from a deep link or the menu bar
- **Window-scoped wave snapshots** — the connected wave snapshot is scoped to the window's repo, so multiple windows don't cross-contaminate
- **`concerto-dev --repo`** — launch the debug build straight into any repo

## Vocabulary: Wave → Loop

A design pass settled the MVP nouns: **Loopflow** (product) → **Loop** (an always-running, steerable session aligned to a **Goal**) → **Worker** (a hosted tmux session running one Flow on one Task) → **Flow** → **Skill**. "Loop" says what the thing is and it's the word in the product name, so it supersedes the earlier "keep Wave" call. The rename is wide — API, `wave/` dirs, Concerto UI, DTOs, config keys, goldens — and lands as its own migration; 0.9.11 fixes the vocabulary, not the rename.

## Fixes and maintenance

- **Cross-worktree git sync** — checked-out default branches stay in sync across worktrees, and `sync_main` no longer reverts just-merged work on overlapping paths
- **`lfd` sqlite health checks** fixed
- **Installer tolerates `--no-interactive`** in shell installs
- **gstack bundled as a namespaced builtin** — simpler skill discovery, third-party content isolated from local skills
- **Wave reorg** — 13 waves collapsed into root / desktop / mobile / workflows, then the release wave added on top
- **`lf op pr -m/--model`** — agent override for the PR skill
- **Voice update** — checkpoint-and-proceed for reversible work, plus design-stage framing
- **token-compress skill** — context compression, with release-note commits grouped
- **Public website imported** and deployed from loopflow
- **Hashimoto-style review ritual** — a standing quality lens (simplicity, operations, API shape, deletable complexity) run before any unit of work is called done
- **Dependabot automation and dependency bumps** — auto-enable squash merge on open, close PRs only on required-check failure, and routine Rust/Python/Swift/Actions updates across the cycle

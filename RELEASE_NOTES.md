# v0.10.0

Loopflow 0.10.0 turns more of the system into explicit workflow: waves can coordinate other waves, ops items can run directly inside flows, and the UI now surfaces the work that actually needs a human.

Changes since `v0.9.9`.

## Waves can coordinate themselves

- **Chords collapsed into ordinary waves** — coordinating work now lives in normal wave configs pointing at `wave/<name>/` directories, with a new `tend` flow for scanning, assessing, drafting, reviewing, and applying changes across member waves.
- **Flow branching is more explicit** — `fork`/`branch` become `and`/`or`, `or` can name a router step, and flows can end cleanly in `silence` when there is nothing to do.
- **Failed work gets a repair pass before escalation** — step failures now create repair runs in the same branch/worktree, and only unresolved repairs bubble up as algedonic signals for a parent wave or human.
- **The step surface is smaller and clearer** — duplicated `.lf/steps/` overrides are gone, builtins are the single source of truth, and gate/review/update-wave/design templates now share clearer structure and measurement hooks.

## Humans see the work that needs them

- **Attention queue replaces scattered block state** — Concerto and `lfd` now surface review-ready runs, step failures, and blocked work in one lifecycle-backed feed, with actions like Ship and Retry plus automatic resolution when the underlying condition clears.
- **Review splits by intent** — `review` is now a routed `demo` or `code-review`, and `review-design` is reframed around reshaping the AI's elaboration into the user's actual intent.
- **Review prompts orient before judging** — design and code review openings now start with what changed, what is open, and where judgment is needed instead of anchoring on whatever looked most "interesting."
- **Concerto starts warmer** — the daemon now starts eagerly, local Claude/Codex auth is detected from credential files, and the redesigned connections panel is grouped by role instead of a flat provider list.

## Connections, auth, and secrets got sharper

- **Provider tokens are encrypted at rest** — SQLite and Postgres token storage now uses AES-256-GCM, secrets are redacted in logs, and existing plaintext tokens migrate automatically on startup.
- **Auth collapses to two modes** — `lfd` now supports `local` and `studio`, both with automatic session-token creation, and the docs/config surface is rewritten around those two deployment shapes.
- **Docker is the explicit remote default** — deploy docs, compose defaults, and install guidance now steer `mode: container` toward Docker plus studio auth, with credential mounts called out as the supported path.
- **Project-management and secrets providers are first-class** — Loopflow adds Asana and Linear foundations, OAuth-based Asana auth plus wave export, and a Doppler-backed secrets provider that can populate Claude/Codex keys from project config.

## More of the mechanical work happens automatically

- **Flows can run `ops:` items directly** — land, rebase, release, and similar mechanical work no longer needs a full agent session, and gate can hand off cached PR copy through `scratch/` for later `lf ops land`.
- **PR and commit copy writes itself more often** — `lf ops pr`, `lf ops land`, and `lf commit` can generate title, body, or message from the diff when you omit flags, with sturdier parsing when agent output is noisy.
- **Rebases and land recover more gracefully** — ops commands can launch a repair agent on rebase conflict, stage uncommitted changes before landing, and skip squash-merged parent commits when rebasing stacked branches.
- **Worktree rotation is less lossy** — freshly rotated branches stay marked fresh after squash merges instead of being pruned immediately, preserved worktrees keep the branch timestamp in their name, and main is synced before create/list operations.
- **Release and install edges got cleaned up** — Concerto resource bundles now live under `Contents/Resources/`, font loading works across app/SPM/Xcode contexts, release signing moved into a dedicated script, installs skip unnecessary sdists, and `lf ops land` fails early if the target tag already exists on origin.

## Less stale surface area

- **Unused config and old artifacts were deleted** — the unused `push` and `include_loopflow_doc` fields are gone, `init` now separates repo and user config guidance, and roughly 13,000 lines of stale artifacts and local copies were removed.

# v0.12.4

v0.12.4 makes Projects responsible for how their Tasks move from intake to shipment, and makes the release host responsible for turning a built tag into an observed production release. Task lifecycles are now explicit, immutable `first → loop → finally` plans; release runs are exact, resumable, and complete only against configured evidence. The built-in catalog now follows the same Task, Project, Wave, and Ops model instead of presenting a second product map.

## Projects define how their Tasks run

A Project can now encode the operating path appropriate to its work. Every new Task resolves that Project policy once, pins the result, and keeps the same lifecycle across restarts and serial PRs instead of inheriting whatever configuration happens to exist later (#1117).

- `lf pm project create` and `lf pm project update` accept `--first`, `--loop`, and `--finally`; the values round-trip through Linear, cached status and roadmap data, Rust DTOs, and the Mac app.
- `lf task start <project> [title]` now requires the owning Project. Piped input is preserved as the Task report, with its first line supplying the title when no title argument is given.
- Per-Task `--first`, `--loop`, and `--finally` overrides replace the single `--flow` override. Loopflow validates all three flows before creating the Linear issue and refuses a later override that conflicts with the pinned plan.
- Projects without an explicit policy use `task-design` once, repeat `slice`, then run `ship`. The final flow may end in mechanical operations; the first and loop phases remain agent work.
- Incident projects can use `incident` to restore service and run 5 Whys, repeat `ship-5whys` for coherent causal fixes, then use `ship` to gate the result, record learnings, and land it. A full incident report can enter directly through stdin.

## The catalog matches the operating model

Built-ins are now organized by the thing that owns the behavior: Task, Project, Wave, or Ops. This makes the names shown by `lf --list`, the lifecycle described by the docs, and the durable runtime hierarchy agree (#1116, #1117).

- Core skills and flows share one flat namespace under those four categories. External catalogs remain namespaced rather than being mistaken for Loopflow's core behavior.
- `code` is the focused `implement → compress` pass; `build` stops after a reviewed Task slice; `ship` owns the final gate, durable learnings, and landing. Static analysis and affected suites belong to the shipping gate rather than being repeated at every coding phase.
- Operational skill names now state their commitment level: `pr-publish`, `pr-submit`, `pr-land`, `rebase-conflicts`, and `release-run`. The mechanical `lf pr ...` command surface is unchanged.
- `lf init` now starts by discovering Homes, agent accounts, Waves, Projects, Tasks, and shared read surfaces. Direct skills remain available, but initialization no longer treats prompt launching as the whole product.
- The previously bundled gstack catalog and retired built-in direction packs are no longer shipped in the binary. Third-party behavior must come from repo-local configuration or a separately installed catalog.

## Releases finish against observable evidence

`lf release run` now owns a portable release state machine while each repository owns its verification, preparation, packaging, signing, deployment, and smoke-test commands. A run selects the exact first-parent commit range, prepares an isolated release PR, tags the merged commit, and waits for the completion evidence configured for that target (#1116).

- Release targets can scope an area and tag prefix, select manifests, run `verify` and `prepare` hooks, name a GitHub workflow, and choose `tag`, `workflow`, or `github-release` completion.
- Commits are the shipped-behavior ledger; matching PRs enrich release notes but cannot hide direct commits. `lf release check` reads that evidence without executing repository hooks.
- No changes is a successful no-op. Open or merged release PRs, existing explicit tags, incomplete successful builds, and incomplete completion evidence resume in place instead of cutting a newer release around unfinished work.
- A repository-owned `publisher` receives the successful hosted build inside an exact-tag worktree. Loopflow verifies afterward that the GitHub Release is published, not merely drafted.
- Long-lived release hosts now reconcile the target's version tags from origin before selecting a range. A remotely repaired tag replaces its stale local object, while local-only tags from interrupted pushes are preserved (#1120).

Loopflow's own release path now uses that split in production: GitHub Actions builds and smoke-tests the four native CLI archives without deployment credentials, while the maintained cron host performs signing, notarization, crates.io and R2 publication, website deployment, and GitHub Release finalization (#1118).

- The host runs a preflight before changing release state, downloads artifacts from the successful tag workflow, and records redacted stage receipts under `.lf/logs/`.
- The website is built from the exact tag and exposes it at `/healthz`. If production does not report the expected tag, the deploy script restores the previous Fly image and leaves the release incomplete for investigation or retry.
- The versioned DMG is uploaded before deployment; the `latest` DMG and non-draft GitHub Release advance only after production proves the tag.

## Operational notes

**Update Task launch commands.** Replace `lf task start "title" --project <id>` with `lf task start <id> "title"`, and replace `--flow <name>` with the appropriate `--first`, `--loop`, and `--finally` overrides. Existing Task rows retain the previous SQLite phase representation internally, so this lifecycle rename does not require a database migration.

**Audit references to removed built-ins.** Repositories using a bundled gstack skill or one of the removed `ceo`, `craft`, `creativity`, `infra`, `ux`, `openclaw`, or `scale` directions must provide or install that behavior separately before upgrading.

**Release configuration remains optional.** Without `release.targets`, the default target covers the whole repository, auto-detects supported manifests, and treats the pushed tag as completion. Configure hooks and stronger completion evidence where the repository needs them.

**Loopflow release operators should reconcile the host schedule.** Run `lf cron sync --wave infrastructure` on the maintained host after installing this version. The daily release job uses host-native GitHub, Doppler, signing, registry, R2, and Fly authority; those credentials no longer belong in the tag workflow.

## Small changes

- The release shell installer accepts `--version <X>` or a positional version to install a specific GitHub Release, while the default remains `latest`; downloaded binaries still activate through guarded `lf install promote`.
- `lf release publish` can stage notes and repeated assets in a draft GitHub Release, then finalize that draft as the latest release.
- Project updates can change lifecycle flows without forcing a replacement definition or KR list.

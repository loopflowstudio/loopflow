# systems wave memory

Steers Loopflow toward boring releases: nightly verification that never deploys, weekly publishing gated on that same verification, and a repo-owned self-hosted `lfd` running the crons — with Cadenza mirroring the cadence.

## Shipped

- **Install syncs skills** — both install paths (refresh and local `--use`) run `lf op sync-skills --global --yes` after installing `lf`/`lfd`, so `~/.claude/skills` and `~/.agents/skills` always track the freshly installed binary. Sync failure warns but never fails the install; the binaries are already in place. First increment of "one command keeps local fresh."
- **Deterministic rebase & placement in `lf op`** (rebase-efficiency parent) — `lf op rebase` now classifies the branch via merge-base diff *before* touching git and picks reset / direct-rebase / rebase-onto-parent / skip-parent-onto-main / noop; only genuinely conflicting authored work escalates to the rebase agent. Disposable branches (no unique commits, generated/checkpoint-only, scratch-only) reset to base instead of burning a long rebase. `scratch/` survives via directory copy to `.lf/tmp/scratch-stash/<branch>-<ts>/`. `lf op wt create` plans placement through `engine/worktrees` (the one naming rule): stacked-from-current by default, `--main` root escape, `--stack PARENT`; user segments with `.` are rejected (dots reserved for ancestry). `--plan` on both prints the deterministic decision without mutating git. Ops telemetry → ignored `.lf/tmp/metrics/ops.jsonl` (strategy/class/counts, no diffs or secrets). Classifier uses merge-base diffing so upstream-only drift isn't counted as local authored work. E2E: `tests/e2e/test_rebase_efficiency.sh`. This directly attacks the "avoidable long rebase" sharp edge in the daily loop.

## Gotchas

- **Dotted-root vs dotted-ancestry collision** — repo config `branch_names.schema: "{user}.{name}.{ts}"` produces dotted *root* branch names that visually resemble dotted stack ancestry (`a.b.c` → parent `a.b`). The rebase-efficiency parent reserves dots only for *new* user placement segments; existing dotted roots stay concrete branch names, not ancestry. The real fix is a config/naming-schema grammar redesign (see Next).

## Model (design settled)

- Self-hosting is the default. The public repo carries containers, deploy scripts, service units, schedules, and docs; secrets live in Doppler or host-local env, never git.
- Nightly verifies release-grade artifacts with no publish or deploy side effects; weekly publishes only after equivalent verification passes in the same run.
- Secure remote execution binds remote `lfd` with `LFD_AUTH_TOKEN` behind TLS/Caddy/Tailscale. Studio auth and hosted discovery are gone.
- Loopflow carries the primitives; Cadenza mirrors the cadence and shape until a product-specific difference is deliberate and documented.
- Don't extract a generic multi-product deploy platform before a second or third real deployment proves the shape.
- Release owns the automation spine, not release-content substance: each product owns its own changelog and provider-specific agent credentials (beyond pass-through/secret wiring).

## Next

- **Drain current buffer** — keep local `lf`/`lfd`, release scripts, and CI aligned with the latest merged release-infra work. Known drift: `wave/*/items/*.md` and `wave/*/[0-9]-*.md` local roadmap mirrors are stale — the roadmap now lives only in Linear (`lf op pm show`), not in local files. Sweep these stale mirrors when a broader wave-hygiene pass runs; this update-wave run left them in place per the skill's "never delete local roadmap files" rule.
- **Cadenza release parity** (items/01) — same nightly/weekly cadence, one-command updater, tests, self-hosted assumptions; document any deliberate divergence.
- **Cron host bootstrap** (items/02) — bring up the first maintained self-hosted `lfd` host (Mac mini + Tailscale default), Doppler configured, root/conductor wave with scheduled checks.
- **Release feedback loop** (items/03) — failed nightly/weekly runs surface as attention items or focused fix PRs, distinguishing verification vs publish vs host vs stale-local drift.
- **Replicate intentionally** — apply the skeleton to Manabot/Hootro only when they need it.

### Rebase-efficiency follow-ups (file to Linear)

- **Config/naming-schema child** — redesign `branch_names.schema` so branch ancestry and root formatting stop fighting over dots; include migration, config docs, DTO/fixture updates if wire shapes change, and prompt guidance. The parent PR deliberately did *not* solve this; it opens as a stacked child.
- **Normal `lf` placement flags** — `lf <flow-or-step> --stack|--fork|--dispatch` execution placement, calling the same `engine/worktrees` planner as `lf op wt create` (not a reimplementation). Parent landed only the shared engine + the `wt create` surface.
- **Land/advance split** — add `lf op land --advance`/`--no-advance`; decide the default after `ops.jsonl` telemetry shows post-land repair cost. `lf op land` behavior is unchanged in the parent.
- **`--fork` is a pure alias of `--main`** today — both resolve to root-off-default in `plan_placement`. Either wire the documented review-base distinction or delete `--fork` + `PlacementRequest::Fork`.
- **`--stack [PARENT]` optional-value parser** collides with positional `NAME`; settle the non-ambiguous stacked-from-current spelling before agents lean on it (awkward forms like `--stack -- name` are the current escape).

### How to judge rebase efficiency (dogfood metrics from `.lf/tmp/metrics/ops.jsonl`)

Local-only JSONL, reviewed weekly. Key product metrics: **agent-rebase rate** (% of rebases launching an agent), **avoidable rebase-agent rate** (stale/empty/generated-only branches that still launched one — target 0), median `land`→queued/merged time, post-land repair rate, and command-drift rate (prompt-recommended commands the installed `lf` can't parse). Then flip one default at a time: stack-by-default `wt create`, stale-empty reset before rebase, land/advance split, generated-only reset policy. Synthetic-workload replay harness (50–100 disposable histories, current vs classifier in trace mode) is unbuilt — file if tuning thresholds needs it.

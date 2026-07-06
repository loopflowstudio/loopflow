# Architecture roadmap access after the Linear PM provider flip

## Problem

`lf op pm show --wave architecture` errors:

```
wave/architecture/GOAL.md has no `pm.asana_project`.
Run `lf op pm init --wave architecture` to connect its roadmap.
```

even though `wave/architecture/GOAL.md` frontmatter is:

```yaml
pm:
  provider: linear
  linear_project: '8c4ba3f9-cf23-4136-87ed-37847aa7dc82'
```

The roadmap is unreadable, which blocks the architecture wave from checking or
updating its own charter. The dispatching wave believed this was a
provider-selection code bug: "make `lf op pm show` honor the configured Linear
provider."

## The finding (this is not a code bug)

The provider-selection path at HEAD already honors Linear. Built the current
source (v0.10.0) and ran the exact command against this worktree:

```
$ ./target/release/lf --version
lf 0.10.0
$ ./target/release/lf op pm show --wave architecture
fetching linear project 8c4ba3f9-… for wave/architecture
open  Collapse lfd/lfq into lf; shrink lfd to a guarded subscription server  id:1e1d2f55-…
open  Unify the operating prompt                                             id:cfa35ff0-…
open  Retire "chord" / "member" — one wave-tree vocabulary                   id:6f75cb7a-…
```

It reaches Linear and lists the roadmap. `resolve_provider`
(`rust/loopflow/src/ops/pm.rs:153`) reads `pm.provider` first, parses `linear`,
and `read_project` returns `linear_project`. Unit tests
(`resolve_provider_selects_linear_from_frontmatter`,
`fetch_items_dispatches_to_linear_provider`) already cover it.

**The failure comes from a stale deployed binary, not the code.** The `lf` that
the wave's shell resolves is:

```
$ command lf --version          # → /Users/jack/Applications/Loopflow.app/Contents/MacOS/lf
lf 0.9.12
```

`lf 0.9.12` predates the Linear provider (added in `0c7b10b2`, default flipped
in `8f34be5e`). It understands only Asana, so it demands `pm.asana_project`.

Two copies are installed; the stale one wins on PATH:

| PATH pos | Path | Version |
|----------|------|---------|
| 14 | `~/Applications/Loopflow.app/Contents/MacOS/lf` | **0.9.12 (stale, shadows)** |
| 20 | `~/.local/bin/lf` | 0.10.0 (fresh) |

The app bundle at position 14 shadows the fresh `.local/bin` copy at 20, so
every bare `lf` invocation — including the wave mind's `lf op pm` calls and
`lfd`'s `resolve_lf_binary()` PATH fallback — runs 0.9.12.

## The demo

```
$ command lf --version        # 0.10.0, not 0.9.12
$ lf op pm show --wave architecture
open  Collapse lfd/lfq into lf; …   id:1e1d2f55-…
open  Unify the operating prompt     id:cfa35ff0-…
open  Retire "chord" / "member" …    id:6f75cb7a-…
```

The wave reads its own Linear roadmap through the deployed binary, not a
freshly built one.

## Approach

Redeploy `lf` (and the Concerto app bundle) at ≥0.10.0 so the copy that wins on
PATH understands Linear. No provider-selection code changes.

```bash
uv run scripts/install.py refresh        # pull default, rebuild lf/lfd, install to PATH
# or, from this worktree:
uv run scripts/install.py local --use    # build this tree, promote onto PATH + /Applications
```

`local --use` also rebuilds the Swift app and can restart the `lfd` launchd
service. Restarting `lfd` mid-run disrupts the very wave server issuing this
work — prefer running the deploy from outside the live wave loop, or use
`refresh` once main carries the fix.

Verify after:

```bash
command lf --version                     # 0.10.0
lf op pm show --wave architecture        # lists the Linear roadmap, or a
                                         # Linear-specific auth error if the
                                         # OAuth token is missing/expired
```

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Does `resolve_provider` honor `pm.provider: linear`? | Yes. Verified by building HEAD and running the command — it fetches the Linear project and lists items. | No code change to provider selection. |
| Is the error from the source or a binary? | Binary. `command lf` is `lf 0.9.12` (Asana-only), predating the Linear commits. Fresh 0.10.0 works. | Fix is a redeploy, not a patch. |
| Which `lf` does the wave actually run? | The app-bundle `lf` at PATH pos 14 (0.9.12) shadows `.local/bin/lf` at pos 20 (0.10.0). `lfd::resolve_lf_binary()` falls back to bare `lf` → same shadow. | Must refresh the app-bundle copy, not just `.local/bin`. |
| Does Linear auth work, or will it fail after redeploy? | `pm show` reached Linear and returned items, so the OAuth token in the lfdb store is present and valid. | Redeploy alone unblocks; no auth work needed. |
| Is a Linear team configured for writes? | `LinearClient::new(token, config.linear.team)` — reads need only the project id (worked); `pm update` create-item may need `linear.team`. Out of scope for read access. | Note as a follow-up if `pm update` later fails. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Patch `resolve_provider` | There is nothing to patch — it already selects Linear. | Would be inventing a fix for a non-bug; wastes the diff and misleads the next reader. |
| Hand-copy `target/release/lf` over the app-bundle binary | Fastest unblock. | Bypasses `install.py`, risks breaking code signing / bundle version stamping; exactly the "do it by hand" antipattern loopflow warns against. |
| Reorder PATH so `.local/bin` precedes the app bundle | Cheap, no rebuild. | Leaves a stale 0.9.12 in the bundle that will resurface for anything resolving the bundle path directly; treats the symptom, not the stale deploy. |
| lfd staleness guard (below) as the fix | Prevents the whole class. | It's a new behavior / design-gated change, not the immediate unblock. Filed as a follow-up sketch. |

## Key decisions

- **No provider-selection code change.** The honest root cause is deployment
  staleness. Report it as such rather than fabricate a patch.
- **Fix the app-bundle copy specifically.** `.local/bin/lf` is already 0.10.0;
  the shadowing bundle binary is the one that must move.
- **Deploy from outside the live wave** to avoid an `lfd` restart killing the
  wave server that dispatched this.

## Follow-up sketch (design-gated, not part of this fix)

The silent failure mode — `lfd`/the wave mind exec'ing an `lf` months older than
the running server — is worth closing. A staleness guard could warn (or refuse)
when the resolved `lf --version` is older than `lfd`'s own version, so a stale
deploy surfaces loudly instead of as a misleading "asana_project is missing."
This is new behavior and belongs at the design gate, not smuggled into a roadmap
unblock. Filed here as a sketch only.

## Scope

- In scope: diagnosis; redeploy `lf`/app bundle to ≥0.10.0; verify roadmap reads.
- Out of scope: any `resolve_provider`/`read_project` code change; the staleness
  guard; `pm update` Linear-team write config.

## Done when

```bash
command lf --version                 # 0.10.0 (not 0.9.12)
lf op pm show --wave architecture    # lists the Linear roadmap items
```

succeed against the *deployed* binary — reproducing the fresh-build result above
without pointing at `target/release/lf`.

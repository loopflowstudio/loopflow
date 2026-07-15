# Rebase blocked — branch already superseded by squash-merge #936

**Status:** rebase onto `origin/main` aborted; branch returned to clean pre-rebase
state (`6e3965543`, matching `origin/oauth-auth-compat`). Not pushed.

## What happened

The rebase applied 3/17 commits, then hit escalating conflicts on
`README.md`, `auth.rs`, `provider_auth/mod.rs`, and `lf/mod.rs`. Investigating
the conflicts (not just the markers) revealed the branch is **stale**, not merely
behind.

## Why this is not a mechanical rebase

`origin/main` has only 3 commits since the merge-base (`81c16868a`). Two of them:

- `92c28d2ca` — **"auth: route repositories through profiles with automated
  Claude browser handoff (#936)"**
- `020b8b853` — "auth: keep profile examples generic (#937)"

**PR #936 is the squash-merge of this exact branch's work.** These 17 commits are
the original unsquashed source; their semantic content already lives in `main` in
final, further-evolved form. That is why the rebase conflicts show `HEAD` (main)
holding symbols this branch only introduces in its *later* commits
(`AccountLifecycleUpdate`, `EmailAddress/HostId/LocalChromeProfile/ProfileId`,
`prepare_provider_account_access_token`, the 3-arg
`drive_claude_browser_authorization`), while the incoming branch commits carry an
*older* design (`ClaudeChromeProfile`, `AuthCommand::Pair`, `set_account_enabled`)
that `main` does not have and does not want.

## The hard blocker: migration-number collision

`git diff 6e3965543 origin/main` shows a real divergence that cannot be
auto-resolved:

| Number | This branch | origin/main |
|--------|-------------|-------------|
| `0.11.009` | `context_pressure.sql` | `profiles.sql` |
| `0.11.010` | `context_input_normalization.sql` | `provider_account_lifecycle.sql` |
| `0.11.011` | `profiles.sql` | — |
| `0.11.012` | `provider_account_lifecycle.sql` | — |

Main renumbered profiles→009 and lifecycle→010 and **dropped** the two
`context_*` migrations. Same migration numbers now carry different content on the
two sides. Reconciling live migration history is not a text merge — resolving it
wrong corrupts the schema-version ledger.

Main also carries unrelated new work absent here: `lf/commands/top.rs` (+371),
`docs/lf.md`, `engine/builtins/LOOPFLOW.md`, and eight `tests/goldens/*` updates.

## Options (need a human decision)

1. **Close the branch / PR as landed.** Its feature shipped via #936. This is the
   most likely correct action — the branch has no unique feature content left, only
   an obsolete design plus a migration numbering that collides with main.

2. **Salvage only the genuinely-unique bits.** If `0.11.009_context_pressure` and
   `0.11.010_context_input_normalization` are wanted features that did *not* ship
   in #936, cherry-pick just those onto a fresh branch off `main`, renumbered to
   `0.11.011`/`0.11.012` to sit after main's profiles/lifecycle migrations. Confirm
   first whether those two migrations are in-flight elsewhere.

3. **Force the rebase anyway (not recommended).** Take `main`'s side on every
   conflict; the 17 commits collapse to empty and the branch becomes equal to
   `main` (empty PR). Risks silently dropping the two `context_*` migrations and
   wastes review on a no-op branch.

Stopping here rather than guessing, per the rebase skill's headless high-risk rule.

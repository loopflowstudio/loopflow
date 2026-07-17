# Open questions and assumptions — W2-283

Three places where the design departs from the directive as written. Each is a
resolved judgment call, not a deferral. Jack's guiding constraint was "enforce
simplicity as long as we make no mistakes or incorrect assumptions" — these are
the assumptions I found and what I did about them.

## 1. `lf auth exec` does not exist (directive assumption is stale)

The directive says "`lf auth exec` (PR #1027) is the same principle for
interactive vendor use." It shipped in #1027 and was **deleted** in the #1029
branch (on main as `0dd1bf843`, in my base). `AuthCommand::Exec` and
`exec_account` are both absent from the tree. Removal rationale, verbatim:
"auth is for ceremonies, not for running agents; interactive vendor use on a
managed account goes through the same selection (`lf -m codex --account <x>
--tui`)."

The installed release `lf auth --help` still lists `exec`, which is likely where
the assumption came from — the binary lags main (a known wave-memory hazard).

**Assumption:** do not re-add it. "Done means … exec … addresses accounts" is
satisfied by `lf --account`, which is the stronger form of the same principle.
Reverse this only if Jack wants the ceremony surface to run agents again, which
main deliberately rejected two days ago.

## 2. Profile tables drop rather than shrink

The directive: "profile tables shrink rather than drop until forwarding is
rekeyed." The design rekeys forwarding in the same PR, so there is no window
where both arrows exist and nothing to shrink toward. Dropping is the CLAUDE.md
position (one implementation, no compat shims for internal state).

**Assumption:** the "until" clause was sequencing insurance, not a requirement to
keep dead tables. If the inversion has to split across PRs after all, the clause
comes back into force.

## 3. Seeding the default route includes Manabot's account

The default route seeds from every `routing_state='automatic'` account, so codex
gets `[jackstah-1066…, engineering, jack-42d…, manabot-eng]` machine-wide. A
repo with no route — including a Manabot checkout — then fails over across all
four, and loopflow work could spend `manabot-eng`, or vice versa.

This is what the directive asks for ("unrouted repos fail over across managed
accounts instead of silently using ambient credentials"), and a visible wrong
default beats invisible ambient spend: `lf route show` names it and
`lf route set` fixes it per repo. `manabot-eng` also sorts last (no
`last_selected_at`) and is at 80% weekly, so PR 3 demotes it further.

**Assumption:** accept it. Ship `lf route default set` in the same PR so the
seeded order is one command away from correct. Revisit if per-repo routes turn
out to be the common case rather than the exception — today the store holds
exactly one route row, which is the whole problem.

## Not a question, but do not skip it

Migration ordinal `0.11.027` is free as of 2026-07-16 (max on main is
`0.11.026`), checked twice: against 4 open PRs while drafting, and against 8 at
review — the count moved by three *during the write*, which is the wave-memory
race in miniature. It is only free at the instant you look. Per wave memory a
sibling landing any migration turns migration-check red on this branch until it
rebases — that failure is staleness, not a defect in the diff. Re-scan open PR
diffs for the *tables*, not just the ordinal, immediately before landing.

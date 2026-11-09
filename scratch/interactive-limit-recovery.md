# Open Task Sessions after a usage limit

November 9, 2026, demo conversation. Jack Heart:

> i have still had the issue that if i have a bunch of task sessions open and they all hit a usage limit i need to restart them all. can we automatically do soething or at least add something in hte desktop so you dont need to go to an obscure cli command

The reported outcome remains unsatisfied. No demonstration or approval of
LOO-811 occurred. During the first demo preparation its candidate binary was
absent and installed lf reported `0.15.2+7e90dafb1.dirty`. A later demo request
started a local candidate build; no installation, account switch or Session
restart was performed.

The first handoff went to LOO-813. Jack Heart then corrected the placement:
"813 was marked complete so move that" and "to 811", enclosing the three targets
below. LOO-811 now owns this recovery outcome. Leave LOO-813 complete; its merged
PR #1863 is prior work, not completion of automatic quota recovery. PR #1862's
two-item Keychain repair is only part of LOO-811's outcome.

- Automatically continue affected Sessions when another login has capacity.
- Show Recover affected sessions in Desktop, with progress and anything still waiting.
- Keep history, drafts and approvals intact.

Desired experience: recover the affected open conversations together when an
eligible login has room; otherwise expose a visible Desktop recovery action and
explain what is waiting. Retain each conversation's history, model selection,
drafts and pending requests. A shared credential switch alone does not establish
that a stopped message continued. Automatic retry must distinguish a rejected
turn from uncertain delivery or completed tool effects.

Provider-specific reproduction remains to be identified; asked whether the
report concerns Claude, Codex or both. No answer recorded yet.

Scope update saved to LOO-811's canonical brief and title. Linear delivery of
the title and brief timed out and remains pending; the correction on LOO-813's
thread synced. Fresh status kept LOO-813 done and LOO-811 at demo, with no live
Graph. The local candidate build (`cargo build -p loopflow --bin lf`, inherited
LF_* unset) passed in 1m 45s. No disposable demo was launched.

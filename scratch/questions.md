# LOO-672 — Decisions after infrastructure

Jack Heart selected:

- October 28: return at the bottom; exact scroll position need not survive release.
- November 2: infrastructure first, then redesign on delivered behavior. Save net
  machine resources while keeping switching among active Tasks fast.
- November 4: keep recent unfinished Tasks warm, accepting memory cost for instant
  input. Completed Tasks still release on departure under LOO-717.
- November 4: takeover without Fork. Keep last-screen + explicit Take back from
  the earlier selected interaction. The intervening shared-input idea is not
  selected. Navigation alone does not transfer input.

Current plan: `loo-672-ready-conversations.md`. The old kickoff plan is preserved
at `research/loo-672-pre-infrastructure-plan.md` as historical evidence, not direction.
Its detached quota, old budget, Claude-only Fork and snapshot-building proposals
are superseded. The CLI fork stays available independently.

Observed November 4: 708/#1802, 709/#1804, 710/#1801, 712/#1800 and 720/#1809 are
done and merged; freshly fetched main is `ac8e088f5f`. 720's candidate Claude/Rex/
Desktop acceptance and CI passed; machine-wide installation and live Codex/
OpenCode proof remain separate. 719's installed macOS check remains unstarted.

717/#1811 is at demo, open, with confirmed-host-exit work still pending. Reuse its
shared retention/retirement owner, modifying Desktop's unfinished release policy
for the warm set. Completed lifecycle remains two minutes for eligible agents and
fifteen total for shells/checkouts, respecting protected work and other hosts.
717 permits discarding eligible uncommitted checkout files; 672 adds no cleanup
or automatic deleted-checkout reconstruction policy. 498/#1814 owns Rex adapter
and occurrence integration and still has delivery gaps.

Provisional engineering choices: eight recent unfinished conversations including
selected ones, no eviction of visible panes, selected-first bounded restore with
two concurrent preparations. Measure private memory and time-to-input. These
numbers are not choices attributed to Jack. Carry automatic-reveal intent into
the actual Rust launch/claim; a stale Swift DTO is insufficient.

No unresolved product question remains after the latest warm-set and takeover
choices. Integration after 717 lands or an explicit stack is still required.
Jack Heart subsequently approved proceeding on November 4: “ok, proceed.”
Implementation and review publication through interactive demo are authorized;
shipping and installation are not. Use the installed entry at
`/Users/jack/.local/bin/lf`: the inherited PATH still resolves the old app-bundled
0.15.1, which cannot read the current registry. Do not migrate or replace the store.

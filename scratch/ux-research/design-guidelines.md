# Concerto UX — design guidelines

What we believe about Concerto's UX, learned from the UX research loop. Each
entry is a claim we'd design against next time, cited to the loop and evidence
that produced it. This file grows every loop. If a later loop contradicts an
entry, mark it revised — don't delete it silently.

---

## From Loop 01 — attention triage on the wave list

**G1 — Status must carry its reason, not just its word.**
A bare status word (`"Waiting"`, `"Failed"`) fails the "which wave needs me"
behavior; every persona in loop 01 succeeded only on candidates that showed
*why* (`Failed at gate`, `Waiting — 3/3 PRs open`). The model already carries
this — `WaitingReason.prLimitReached(open, limit)`, the failed step off
`activeRun`, `iteration`, `diffIndicator` — and the shipped
`RepoSidebarWaveRow` throws all of it away. Rule: a wave row's second line is the
*reason for its status*, drawn from real model data, never a restatement of the
status word.

**G2 — Rank by attention; never show waves in insertion order.**
The shipped list preserves registry order, so a `failed` wave can sit below three
`running` ones. Unanimous across personas: attention-needing waves float up.
Rule: default order is `failed → waiting → running → idle`, everywhere waves are
listed, regardless of layout.

**G3 — Make *calm* legible, not just alarm.**
The behavior includes the negative case: "nothing needs me" must be readable in
~1 second. Winning designs made calm a positive signal (collapsed green "all
clear" line, an empty "Needs You" column, a green palette top-row) rather than
"absence of red." Rule: design the zero-attention state as a first-class,
glanceable state.

**G4 — Repo is a filter, not the primary axis — and the shipped sidebar
over-weights it.**
"Repo is a filter, not a container" (roadmap guardrail) is under-served by the
current repo-sidebar-first layout: it visually says "pick a repo first," which
fights the cross-portfolio "what needs me" glance. Every candidate that demoted
repo to a filter (A band, C board) scored better on the target behavior than the
one that kept the sidebar primary (B). Solo users see the sidebar as dead weight;
the team lead triages faster without it. Rule: attention is the primary axis of
the default surface; repo is a filter you reach for to browse. (Revisit if a
future loop finds repo-as-place indispensable for large portfolios.)

**G5 — The list routes attention; it must not become a detail surface.**
Opening a wave lands in the terminal-first wave screen (harness pane + yazi +
terminals + RepoWork strip), never a rendered detail panel — "frame, don't
render." The row's job ends at *route me to the right wave*; enriching the row
(G1) is legibility, not the beginning of an in-list detail view. Rule: rows carry
status + reason + an explicit open affordance, and stop there.

**G6 — A keyboard path is non-negotiable for the power user, but it's an overlay,
not the base.**
The command-palette (D) was the only candidate that beat Terminal.app for the
terminal-first power user, *and* the only outright fail for the team lead and the
newcomer as a primary surface (hidden keystroke = no ambient health, no
discoverability). Rule: ship a `⌘K` attention-sorted jump-to-wave palette as an
overlay on whatever the default visual surface is — never *as* the default.

### Open decision carried out of Loop 01
The **default visual surface** is unresolved: **A (attention band/sections)** —
the safe consensus that passes all personas — vs. **C (portfolio board)** — the
team-lead-optimized pre-attentive glance that fails the solo/power personas. B
(enriched list) remains the minimal-change fallback. D (palette) rides on top of
whichever wins. See `questions.md`.

### What the next loop should test
Loop 02: resolve A-vs-C for the default surface by pressure-testing both at
**real portfolio scale and at the empty/1-wave extreme** (the two places they
diverge most), with the row-reason legibility (G1) held fixed. Concretely: does
C's board still glance-win at 30 waves once the IDLE column is a wall, and does
A's band degrade gracefully to a single line for the 1-wave solo user? Bring
scale into the personas' scenarios rather than assuming it.

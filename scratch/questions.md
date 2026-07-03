# Open questions — waves one level out

Deferred forks. Do NOT resolve these in the slice-1 run; they're recorded so the
wave-list build doesn't accidentally decide them.

## Shared vs per-repo flow (decide when we open a wave — slice 3+)
Identity (GOAL/MEMORY/agent) is singular either way. Open: does a wave run one
flow across all its repos, or can each repo carry its own flow?
- Lean: **shared intent, per-repo status.** One flow/goal; each repo reports its
  own status/iteration/PR; wave status is a rollup.

## Wave/RepoWork model split (slice 2+)
Slice 1 stubs `repos = [wave.repo]`. The real split moves per-repo fields
(worktree, branch, status, iteration, activeRun, commits, diffStat, openPRCount,
pr) down into `RepoWork`, and it's a wire-type change across Rust + Python +
Swift + DTO fixtures. Sequenced after the UI shape is proven.

## Multi-user / cloning (way down the line — do not build)
Singular identity makes a wave shareable as a team/project. Cross-user sharing,
when it comes, is likely **cloning** (copy identity into another user's space,
their fired state fresh) — not live shared session state. Keep firing-state
separable from identity so this stays open. Not this quarter.

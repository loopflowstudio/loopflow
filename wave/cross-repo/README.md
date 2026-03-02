# Cross-Repo: Parent/Child Repos

## Vision

Two layers:

**lf** already works with arbitrary paths. `lf implement -a /other/repo/src/` loads context from another repo today. There's no parent/child protection at the lf level — you can request whatever areas you want, and lf will read/write wherever you point it.

**lfd** adds the DAG. The portfolio is a set of repos as nodes with directed parent->child edges. lfd maintains these relationships, enforces cycle detection, and auto-resolves child repos by name. This makes cross-repo ergonomic — instead of spelling out full paths, a parent session gets its children resolved automatically.

| Layer | What it knows | What it does |
|-------|--------------|--------------|
| lf | Paths | Accepts any area path. Reads/writes anywhere. No graph awareness. |
| lfd | DAG edges + repo identity | Resolves children by name, injects related-repo areas, enforces cycles. |
| Concerto | UI over lfd | Manages edges visually, presents cross-repo areas and trigger targets. |

Edge direction determines how lfd sets up sessions — what context it injects, what volumes it mounts in Docker, what paths appear in the agent's workspace:

| Direction | Context | Docker mounts | Listen |
|-----------|---------|---------------|--------|
| Parent -> Child | Child areas + docs | Child repo R/W | yes |
| Child -> Parent | Parent docs | Parent repo R/O | yes |

This is lfd state, not repo config. Managed through Concerto. No cycles.

Repos are identified by GitHub short URL (`owner/repo`). The `Repo` type gains a `RepoId` field derived from the git remote.

### Not here

- Repo config for parent/child (it's purely lfd state)
- Auto-discovery of children (explicit edges only)
- Cross-repo atomicity (commits succeed or fail independently per repo)
- Access control at the lf layer (lf takes paths, period)

## Strategy

lf already takes arbitrary paths — cross-repo works today at the file level. lfd adds the graph: explicit parent->child edges between repos, auto-resolution of related-repo areas into sessions, and cycle detection. Concerto manages edges visually. Commits stay per-repo (no cross-repo atomicity). Repos are identified by GitHub remote (`owner/repo`).

## Goals

- lf accepts cross-repo area paths (already works, may need polish for commit handling)
- lfd portfolio model supports directed acyclic edges between repos
- lfd auto-resolves related repos into session context, making cross-repo seamless
- Concerto UI for managing edges, cross-repo areas, and cross-repo trigger targets
- Studio is the first consumer, with loopflow as child

## Risks

- Cross-repo commits: a session modifying files in multiple repos needs separate commits per repo. No rollback — a failure in one repo is reported, not compensated.
- Context budget: loading docs from multiple repos could blow token budgets. Related repo docs share the area budget.
- Worktree interaction: parent worktrees need to resolve child paths correctly.
- Repo identity: requires a GitHub remote to derive `RepoId`. Repos without GitHub remotes can't participate in edges.

## Metrics

- Cross-repo context resolution latency: seconds to resolve child repo areas into session context (target: <2s)
- Number of cross-repo commit failures per week due to split-repo atomicity (track to validate design)
- % of cross-repo sessions where user manually specifies paths vs auto-resolved (adoption rate, target: <10% manual)
- Single-repo test suite pass rate before and after cross-repo changes (regression detection, target: 0 regressions)

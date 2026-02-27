# Cross-Repo: Parent/Child Repos

## Vision

The portfolio is a DAG. Repos are nodes, parent→child relationships are directed edges. A parent can access its children's filesystem, docs, and context. The child doesn't know it's a child.

This is lfd state, not repo config. Managed through Concerto. No cycles.

### Not here

- Repo config for parent/child (it's purely lfd state)
- Bidirectional relationships (edges are directed, parent→child only)
- Auto-discovery of children (explicit edges only)

## Goals

- Portfolio model supports directed acyclic edges between repos
- lf sessions in a parent can read/write/commit to child repos
- Child repo docs flow into context when relevant
- Concerto UI for managing edges and viewing cross-repo waves
- Studio (private monorepo) is the first consumer, with loopflow as child

## Risks

- Cross-repo commits: a session modifying files in multiple repos needs to produce separate commits. Getting this wrong could leave repos in inconsistent states.
- Context budget: loading docs from multiple repos could blow token budgets. Need clear strategy for how child docs share or get their own budget.
- Worktree interaction: parent worktrees need to still resolve child paths correctly.

## Metrics

- Studio sessions can access loopflow context without manual setup
- Cross-repo waves produce clean per-repo commits
- No regressions in single-repo workflows

---
name: loopflow
description: Launch agents, run Skills and Graphs, manage Sessions, worktrees and PRs with Loopflow; use Task and Wave APIs when requested or already associated.
---

# Use lf

```bash
lf --version
lf help <command-or-skill>
```

Install only when requested. Use the requested Skills, Graphs, worktree and PR
operations, following the user's tools and repository conventions.
Do the requested work in
this checkout; preserve existing contributions and authored Graph boundaries.

For work using Tasks, Waves or Projects, read `lf help skill work` in this conversation.
That skill is the work API contract, also preloaded for work-associated launches.
Continue an existing Task when one is supplied; unrelated repository Tasks do
not select work. Operator skills are explicit procedures, not a consequence of
having a Task. Desktop and custom clients use the same work APIs.

An external harness acts for its user; launching through lf supplies Session and
process identity. Never invent Work identity or claim another caller's authority.
Answer here rather than creating another Session to reach the user. Use supplied
names in persisted artifacts and preserve unknown attribution.

Command and skill help is the current reference. Documentation is available at
https://loopflow.studio/docs/agent-api.md and https://loopflow.studio/llms.txt.

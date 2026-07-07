---
pm:
  provider: linear
  linear_project: 'fbdd6124-6114-4427-b6ac-5788dead4f87'
---

## Objective

Turn a Wave from a ticker spawning cold, stateless runs into a persistent Looping
Agent running against a Goal, steered by a live Linear roadmap — until writing goals
is a good way to compute, with clients and servers built from goals and zero step
authoring.

## Measures

- **Key Results**: reference builds from goals, zero step authoring: 3 (mobile client, CLI client, server) reaching demoable. Target 3/3.
- **Key Results**: product-dev step-authoring rate on those builds. Target 0.
- **Key Results**: unattended loop iterations without human intervention. Target >= 20 consecutive.
- **Quality**: Linear round-trip stays complete — items the loop reads are also written back with status.
- **Quality**: Concerto can launch and show looping sessions per repo.
- **Done means**: a landed PR of real product code, roadmap item closed and PR-linked.

## Process

Read the live Linear roadmap each loop. Size before routing: mechanical changes go
direct to a worker in a fresh worktree; unclear or cross-cutting moves get a
scratch design doc and review pass first. Routing lives in this judgment, not in
frontmatter. If a landing would not ship real product code, it is not ready.

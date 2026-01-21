---
choice: add_to_roadmap
reason: No roadmap items exist yet
options: [add_to_roadmap, scope_from_roadmap]
---

The `.docs/roadmap/` directory doesn't exist, which means there are no roadmap items to scope from. Before we can run the design → implement → polish pipeline, we need to first add items to the roadmap.

The `add_to_roadmap` branch will fork to multiple models with different voices to propose roadmap items, then join the results. Once roadmap items exist, a future run of the ship flow can choose `scope_from_roadmap` to design and implement one of them.

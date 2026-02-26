---
name: direction-clarity
description: Design around data structures and public APIs. 1:1 mapping between real-world concepts and code.
loopflow: true
user-invocable: false
---
Design around data structures and public APIs. 1:1 mapping between real-world concepts and code.

Code demonstrates its own correctness. If a feature exists, a test proves it works.

- Name things after what they are: Document, FileEdit, Target — not DocumentHelper, EditResult, OutputHandler
- Aim for a reader to understand the system by reading the types and their relationships
- Make it easy to see what's done and what's broken
- One source of truth per concept

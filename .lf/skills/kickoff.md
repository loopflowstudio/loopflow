---
extends: builtin
---
## In this repository

**Read the code's history before patching it.** Find the subsystem's earlier
Tasks and PRs (`git log` on its files). If it was built for a replaced model,
or this would be its third patch, plan a redesign instead and say so.

**Check every sibling variant.** A fix framed from one provider, harness,
tracker or host shares code with its siblings (Daytona beside Modal, Asana
beside Linear, every harness). Check each variant for differing assumptions
and cover it or note why it doesn't apply.

**A replacement keeps what the old code covered.** When swapping an engine,
list its languages, formats and cases and keep each one.

**Check the blast radius across every surface.** Name what the change does to
each of CLI, Server RPC and schema, Storage and migrations, DTOs and fixtures,
Desktop (`swift/LoopflowMac` + shared `swift/Loopflow`), iPhone
(`swift/LoopflowiOS` + shared `swift/Loopflow`), and Docs — or say it doesn't
reach it.

**Read `DESIGN.md` and `design/prototypes/` first for UI work.** Tasks touching
Desktop, iPhone or the website start from the design system and existing
prototypes before designing changes.

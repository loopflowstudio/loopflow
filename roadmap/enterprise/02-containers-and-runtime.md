# Enterprise Roadmap: Containers + Runtime Isolation

Define how Loopflow runs agents in a managed environment.

## Goal
Establish a container-first execution model that works for Python or Rust engines.

## Scope
- Container image strategy for agents
- Execution isolation (per run, per tenant)
- Resource limits and scheduling

## Decisions to make
- Worker model: long-lived pool vs per-run job.
- Storage: ephemeral vs persistent workspaces.
- Image build: prebuilt vs user-provided.

## Success criteria
- A working containerized run path.
- Resource caps enforced in practice.
- Clear docs for user-provided images.


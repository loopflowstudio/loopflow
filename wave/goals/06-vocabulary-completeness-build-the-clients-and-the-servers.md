---
priority: medium
asana_id: '1216257471904678'
---
# Vocabulary completeness — build the clients and the servers

**Finish line:** A product developer builds a mobile client, a CLI client, and their server from goals alone, with **zero step authoring**.

## Context

The builtin vocabulary is strong at *iterating on existing code* (design, implement, code-review, demo, qa, debug, gate, deploy) and weak at *originating and running a product*. These gaps are the acceptance test for "expressive enough." If a product dev has to crack open a step, the vocabulary failed.

## What to shape — the missing atoms

1. **scaffold** — stand up a greenfield project (mobile app, server, CLI) from nothing. Everything today assumes an existing repo + design doc; `init` only sets up loopflow.
2. **run** — build & run the artifact: simulator (mobile), start server + hit endpoints, run the CLI. `demo` narrates but doesn't run; `verify` is a Claude Code skill, not a loopflow step.
3. **integrate** — exercise a client against its server (the defining seam of a client/server product). Pairs with cross-repo Goals.
4. **platform build/test** — mobile simulator build; UI/visual review (promote `rams` from skill to step).

## Done when

- Three reference builds (mobile client, CLI client, server) reach a running, demoable state driven only by goals, with 0 steps authored by the developer.
- Vocabulary gap count against the acceptance test = 0.

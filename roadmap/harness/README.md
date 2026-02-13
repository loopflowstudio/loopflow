# Harness Roadmap

Build a memory-first wave chat runtime with explicit message streaming, durable memory, token-bounded history, and an agent harness that runs alongside wave runs.

## North Star

After this roadmap:

- `lf-agent` runs turns with model + tools directly (no external chat CLI dependency)
- user-visible output is explicit `send_message` tool calls (`progress` and required `final`)
- memory persists across turns (per-edit durability)
- filesystem effects are ephemeral by default
- Swift UI streams chat messages and shows memory blocks
- Python client is the fast local testing surface

## Sequenced projects

| # | Project | Commit slices | Depends on |
|---|---------|---------------|------------|
| 01 | [Foundation Contract](01-foundation-contract.md) | 3 commits | none |
| 02 | [Persistence + Token History](02-persistence-token-history.md) | 3 commits | 01 |
| 03 | [lf-agent Skeleton](03-lf-agent-skeleton.md) | 3 commits | 01 |
| 04 | [Anthropic Model Adapter](04-anthropic-adapter.md) | 2 commits | 03 |
| 05 | [Tools + Turn Loop](05-tools-and-turn-loop.md) | 3 commits | 02, 03, 04 |
| 06 | [lfd Process Integration](06-lfd-process-integration.md) | 3 commits | 02, 05 |
| 07 | [Python Local Client](07-python-local-client.md) | 2 commits | 06 |
| 08 | [Swift Client + Viewer UI](08-swift-client-viewer-ui.md) | 3 commits | 06 |
| 09 | [MemGPT/Letta Exploration](09-memgpt-letta-exploration.md) | 3 commits | 06, 07 |
| 10 | [Compaction Rollout](10-compaction-rollout.md) | 3 commits | 09 |
| 11 | [Hardening + E2E](11-hardening-and-e2e.md) | 3 commits | 06, 07, 08, 10 |

Total: ~31 commits, each targeted at a few hundred LOC.

## Non-negotiable system invariants

- Memory is durable across turns.
- Filesystem side effects are ephemeral by default.
- Prompt context is memory + current message + token-bounded harness history.
- `send_message` is the only user-output mechanism.
- Successful turns emit exactly one `send_message(phase="final")` and may emit `0..∞` progress messages.
- Chat lane runs alongside wave runs in its own executor lane (container or process, matching executor type).

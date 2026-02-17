# Open questions

- `scratch/` was empty at kickoff time (no ingested item file). Assumed the next harness backlog item is **A2 — Multi-turn with harness events** from `wave/harness/README.md`.
- Chat turn streams are currently kept in-memory (`HttpState.chat_turns`) without persistence/TTL pruning; if product needs replay across lfd restarts or bounded retention, we should add explicit policy in follow-up.

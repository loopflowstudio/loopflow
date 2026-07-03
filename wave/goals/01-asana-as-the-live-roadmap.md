---
priority: high
asana_id: '1216257840963483'
---
# Asana as the live roadmap

**Finish line:** A Looping Agent reads its backlog live from Asana and writes status back; Asana is the steering wheel, not a one-way mirror.

## Context

`pm.rs` today mirrors Asana/Linear *down* into local `wave/<name>/*.md` and `ingest` picks from the mirror. The Goals model inverts this: the loop reads Asana directly, so reordering tasks in Asana changes the next iteration's priority. Existing seams: `pm_pull`, `pm_try_claim`.

## What to shape

- **Read:** loop pulls the roadmap from Asana each iteration (with sane caching for the hot loop — network in the loop is a risk).
- **Write-back:** finishing a task moves it to Done / comments the PR link; decide the depth (Done + PR link minimum).
- **Discovered work:** the loop may create Asana tasks it finds, so a human can see and reprioritize.
- **Grain (decide at build):** Goal ↔ Asana project or portfolio? item ↔ task or subtask?
- **Blocks → human:** when stuck, surface as an Asana task assigned to the human and/or a Concerto block.

**Target seam:** the loop reads the roadmap through a provider handle, not the local mirror. The future shape is a small Roadmap service the Wave agent calls through lfd/lfq — `list()`, `claim()`, `create()`, `update()`, `complete()` — with Asana as the first backend. Local markdown may stay as a fixture / import-export path, but the hot loop reads provider-backed state. Don't build the full service in this item; wire Asana behind that seam and keep the surface minimal.

## Done when

- A Goal completes a real Asana task end-to-end: reads it, does the work, moves it to Done with a PR link, with no local mirror in the path.

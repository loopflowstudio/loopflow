---
kind: mode
pipeline: "ship"
---
Look at the roadmap and recent git history. Decide what mode to operate in.

## Where to read

- `roadmap/` — vision, architecture, direction
- `roadmap/` — all planned work across all areas
- `roadmap/<your-area>/` — items you own

## Decision

- Substantial approved items waiting? **Build** one.
- Major recent changes or bug churn? **Simplify** first.
- Direction unclear? **Roadmap** — propose new work.

Bias toward building if good work is queued. Only roadmap when you need direction.

## How to decide

Check `roadmap/<area>/` for approved items. If there are approved items, pick the one with the most leverage—where's the biggest gap between vision and implementation?

If the roadmap is thin or unclear, switch to roadmap mode: read `roadmap/` to understand direction, be honest about what's working and what isn't, then propose concrete items to `roadmap/<area>/`.

If recent git history shows lots of changes or bug fixes churning, consider a simplify pass first to stabilize before adding more.

## Context

You see the whole roadmap across all areas. Spot dependencies, avoid duplicate work, understand the big picture. Your `area` determines what you're responsible for building.

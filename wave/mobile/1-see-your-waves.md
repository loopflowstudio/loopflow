---
asana_id: '1214270115553090'
---
# See your waves

**Finish line:** The iOS home tab shows a scrollable list of waves with live status: name, current step (if running), last activity, open PR count, attention count. Pulls down to refresh, updates real-time via WebSocket.

## Context

The daily habit: glance at phone, know what's happening. Wave card = status indicator + headline + counts. Tap to drill in.

## Daily experience

Morning on the train. Pull out phone, open app. List of 6 waves. Two green (running), one yellow (blocked), three idle. The yellow one is `workflows` — blocked on a CI failure. You don't act yet; you know to open the laptop at the office.

## Done when

- Wave list renders all waves with status
- Pull-to-refresh works
- Live updates stream via WebSocket (no polling)
- Empty, error, and loading states all designed
- Health signal per wave visible at a glance

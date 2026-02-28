# Open questions / assumptions

- **Timeseries period format:** Implemented as `YYYY-MM-DD` for day/week (week uses Monday start date) and `YYYY-MM` for month buckets. If we want ISO week keys (e.g. `2026-W06`) instead, we should align API + client parsing together.
- **iOS (iPad) analytics entry point:** iPhone uses a root `TabView` Analytics tab; iPad split-view uses a top-right Analytics toolbar button that swaps the detail pane to the analytics dashboard. If product wants the analytics item directly in iPad sidebar navigation, we should add a dedicated sidebar row.

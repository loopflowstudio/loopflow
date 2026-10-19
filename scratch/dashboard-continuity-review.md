# Dashboard continuity review — October 18–19, 2026

Full record (observations, evidence links, launch details):
`08253c4dda:scratch/dashboard-continuity-review.md`. Evidence JSON remains in
`dashboard-continuity-evidence/`.

## Native review at `e08cbb102d` (synthetic Work, no provider)

Proved in the real app with shared lfdb readers: Dashboard `review` beside the
ordinary Product view; hide collapses to the sibling split; resize 50→52% persists;
restore returns the original 33% outer split; entering a Task and returning keeps
the composition; with autosave off, an unsaved Files draft is held in the shared pane
record and recovered with hidden occurrence/split/tab by a fresh process on the same
lfdb. Zero Sessions and Task admissions throughout. Not proved: same-identity unsaved
badge, live Session/shell continuity (LOO-416), duplicate promotion, stale-write
conflict, anonymous-query arrangement, candidate CLI↔native parity (control targets
`com.loopflow.mac`).

## Jack Heart's walkthrough decisions

- Golden-ratio defaults: Jack prefers them (“group A : group B is a golden ratio”)
  but called the fixed equal-column default “ok i guess”. Unadopted, unrejected.
- Tile fill defect fixed (tiles fill and top-align; secondary “No primary Session”).
- “make the headers stil lthe same regardless of single or dashboard view” and
  “need to be able to "zoom in" to a taks”: implemented as shared `TaskHeading` and
  `Destination.zoomed(dashboard:work:)`; trail goes through the Dashboard (`64efb5f7bf`).
  Open: a `lf desktop` equivalent for zoom, and whether inspection reports it.
- Hidden chips show the Task name only (`0d750ee778`).

## Jack Heart's release notes on 0.14.7 (October 18, later afternoon)

1–2. “Updating...” stuck and “no working tests found”: one failure — the first
   machine-wide planning frame (8.2 MB, 1,233 Tasks) exceeded Desktop's 10 s reader
   timeout. Jack: “Ok, this needs a performance test” / “and needs to be optimized”.
3. Repo chevron: resolved by the agreed jump bar ([agreed-ux.md](agreed-ux.md)).
4. Unstarted Task with no Session view: resolved by the agreed Start offer.
5. Open: “queries have two purposes: what shows in teh sidebar and waht shows in
   the main pane” / “probably need to look more at the sidebar version first?”

## Planning frame performance — October 19

VACUUM copy of Jack's lfdb, 621 Tasks / 52 Waves / 55 checkouts, debug build:
4.70 s, 135 git, 8.27 MB (`5e08b6655f`) → 0.91 s, 59 git, 4.21 MB after
`b2a7d855a0` (latest Flow per Task in one pass), `c62099c2d9` (HEAD/Git layout
from disk), `f03d7cd21d` (roadmap writes each Task once). Proof tests:
`every_task_latest_flow_matches_its_own_flows`,
`planning_asks_git_once_per_live_checkout`,
`planning_runs_git_per_repository_not_per_wave`. Pre-existing failures also on
`5e08b6655f`: `desktop_commands_do_not_route_to_the_execution_machine`,
`released_work_upgrades_to_saved_intent_without_rewriting_work`.

30 leaked `ship` Waves on deleted temp dirs came from a since-deleted test
(`22e34abc54`…`418e52ba65`) resolving the ambient registry; cleanup is Jack's call,
nothing deleted. Machine-wide frame kept: repository scoping saves ~35%; remaining
bytes are mostly descriptions (1.47 MB) and PR evidence (0.75 MB). Open: send
descriptions on demand or bound completed Tasks before scoping.

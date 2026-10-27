# LOO-594 open product choices (for Jack's review)

Each has an assumed default in `scratch/loo-594-refine-the-desktop-wave-view.md`.

1. **Footer width.** Assumed: the footer spans the full window ("wide"), so the Wave
   conversation keeps only the top 61.8%. Alternative: footer under the standup
   board only, conversation full height on the right.
2. **Tile face.** Decided (Jack Heart, Oct 26): live Session view, as on Dashboards.
3. **"Click to expand the Task (and its Task section)."** Assumed: expanding enters
   the Task in full with a crumb back to the Wave, and the sidebar selects that
   Task's row. Sidebar selection inside a Wave view also enters through the Wave
   (LOO-576's Dashboard rule extended). Alternative: expand in place inside the Wave
   view (footer grows) without leaving it.
4. **Standup columns and people.** Decided (Jack Heart, Oct 26): Done since the
   previous working day · Active · Backlog. Done and Active rows carry a
   small profile picture; start with GitHub pictures and names. Skip them for a solo
   player. Jack: "not exactly sure right way to determine that" (solo), and unsure
   whether GitHub or the current Git email is the best identity.
   Proposed (draft, for review):
   - **Identity:** the GitHub user. Done/Active Tasks with a PR: the PR author
     (`author { login avatarUrl name }` added to the PR query lf already makes).
     Claimed Tasks without a PR: the claim's Git email resolved once per email to a
     GitHub login through the repository's commits (`commits?author=<email>` →
     `author.login`), cached in the store. Unresolved: the claim's name and initials.
   - **Solo:** derived, not configured. Show pictures only when the Wave's Done and
     Active rows span two or more distinct people. A solo repository never
     shows them; the first teammate's Task turns them on.
   - Alternatives for solo: repository collaborator count (`gh api …/collaborators`;
     counts people who never touch the Wave), or a setting (a knob to forget).
5. **Done window.** Decided (Jack Heart, Oct 26): a configurable filter, just a number
   of days, default 7. Reuse the existing prototype: `TaskHistoryFilter`
   (`swift/LoopflowMac/TaskHistoryFilter.swift`, default 7, `editDays`, 0 = all ages,
   validation keeps the previous range) and its inline control `TaskHistoryControls`,
   today used only by the Portfolio `WaveDetailPane`. The Done header reads "Done"; hover
   discloses "last 7 days"; clicking it edits the number in place. It carries
   the control; its `showCompleted` toggle does not apply there (Done always shows
   completed). Decided (Jack Heart, Oct 26): the number is sticky
   locally, not shared for now: one value kept in this Mac's app defaults
   (`@AppStorage`), surviving relaunch, never written to the store or Linear.
   Assumed: one value for all Waves, not per Wave.
6–7. **Board and footer addressing.** Decided (Jack Heart, Oct 26): both get
   addressed; "evolve the APIs to be flexible enough to adapt to different views over
   time." Proposed shape in the design's "View parts" section (draft, for review).

## Accepted October 26, 2026 (Jack Heart: "sounds good, lets build")

Choices still open at approval take their drafted defaults:
- Footer spans the full width; the conversation keeps the top band.
- Expanding a tile enters the full Task with a crumb back to the Wave and selects its
  sidebar row (sidebar-entry-through-Wave waits for LOO-576 to land; build the rest first).
- People identity and solo rule as proposed in #4.
- Merged-but-open Tasks show in Active marked "merged" (no `merged_at` wire field).
- `capture -p board` returns the rendered text, like other captures; agents wanting
  structure read `lf wave show --json`.
- Done days: one local sticky value for all Waves (`@AppStorage`).

## Implementation assumptions (October 26, 2026)

8. **People identity, first cut.** The roadmap carries no PR author and lf makes no
   GitHub PR query on this path, so #4's GitHub login/avatar needs a new GitHub
   read and a store cache. Built now: the Task owner's claim name (`runtime.owner`)
   with initials, and the derived solo rule (two or more distinct people across
   Done and Active). GitHub pictures stay a follow-up.
9. **Board rows open their Task.** Jack's LOO-576 steer (70896f61) makes the Wave
   view the place unstarted Tasks are seen and started, so rows enter the Task
   through the Wave like a tile does, instead of being read-only. `+N more` still
   enters nothing.
10. **`tiles` accepts only `page`.** `enter TILE` and `move -p N` already address
    tiles directly; listing them on the part would invite `move -p tiles`, which has
    no meaning.
11. **Done editor.** The Done header uses `TaskHistoryFilter.editDays` for
    validation in a small SwiftUI field; `TaskHistoryInlineView` brings a
    show-completed checkbox that does not apply to Done.
12. **Repository page as a Wave view** — built (October 26). Its board buckets every
    repository Task; its strip is the repository plan's members (Working Tasks,
    started only), since that plan is also the sidebar, which stays active-only.
    Unstarted Tasks are seen and started from Backlog rows, not tiles.
13. **Repository plan slots.** The repository built-in plan now defaults to
    `strip(3)`, so `lf desktop state` reports what the repository page shows. The
    Working Tasks Dashboard renders the same plan, so it now pages three across
    instead of the golden seven. Alternative: keep Working Tasks golden through a
    separate built-in selection, or delete the Working Tasks Dashboard now that the
    repository page shows the same Tasks.
14. **Entering through the repository page.** Selecting a sidebar Task from the
    repository page enters it with a crumb back to the repository page
    (`entered(from: repository)`), as Wave views and Dashboards do; the Task's
    Wave crumb still shows.
15. **Working Tasks Dashboard vs repository page (open, concept review Oct 26).**
    Both render `builtInPlan(.repository)`; one `slots` serves both, so Working Tasks
    now pages three across. Delete Working Tasks, or keep it and move strip geometry
    from the plan to the Wave view? See the design's "Concept review" section.

## Decided October 26, 2026 — Dashboards (answers #15)

Jack Heart: "we dont need the custom dashboards now", "we can just stick with this for
now", "i think we should keep the query system", "but we dont need to expose an entry
point in the ui".
- Keep the query system: saved Dashboards, Working Tasks, `lf desktop query/view/pin/
  arrange/slots/order`, their store tables and `lf desktop state` reporting. No
  migration, no CLI removal.
- Remove Desktop UI entry points to Dashboards: the Dashboard picker/destination
  menu (`DashboardView.swift:37` area), sidebar Dashboard rows or pins, ⌘K/menu items
  that open a Dashboard or Working Tasks, "zoom back to Working Tasks" affordances.
  Rendering a Dashboard selected through `lf desktop` keeps working.
- The repository page's three-tile strip sharing `builtInPlan(.repository)` with
  Working Tasks is accepted; no separate strip geometry.
- Built October 26: sidebar Dashboards section (Working Tasks row, pins, New
  Dashboard), Dashboards in ⌘K and the repository › jump menu, the View ▸ Working
  Tasks item and clickable Dashboard crumbs are gone. The ⌘1–9 menu stays as
  **Go to Task** (it selects sidebar rows, not a Dashboard). `working-tasks`/`goto`
  actions stay for `lf desktop`. A Dashboard's crumb is plain text; Back returns to it.

16. **Show… placement and fresh Tasks (hide/Show…, Oct 26).** Assumed: the Task
   header is the titlebar crumb (`Show…` follows the Task heading); on a Wave view it
   is the board header, moving to a slim `WaveViewHeader` row while the board itself
   is hidden. A fresh Task's empty Shell/Browser start hidden, so its `Show…` lists
   them, matching `hidden_panes`. A lone visible pane has no strip and cannot hide
   (the last pane stays). Board/tiles hidden state lives in navigation, not saved
   across launches.


17. **"Being worked on" (strip, Oct 26).** Assumed: Working Tasks (idle, active,
    moving), the sidebar's rule, now Rust's `wave:` rule too. An idle Task may have no
    live process; Shipping Tasks (a live Graph) stay off the strip and sit in
    Autonomous. Closed Oct 27: Jack Heart chose Working only.
18. **Esc zooms out only a board or strip.** A zoomed conversation keeps Esc for its
    terminal; the crumb's button or parent crumb zooms it out.
19. **Done stays planning-completed** (`task.isSuccessful` within the window), not
    `runtime.status == done`, so Tasks closed in planning without lf runtime count.
20. **GitHub people load once per repository** per Desktop launch; a new owner's
    picture appears after relaunch. Failures leave initials, silently.
21. **Reader startup limit (Oct 27).** Assumed 60 s before a reader's first frame
    (any reader on `LocalLineObservation`), 10 s after. A reader hung at launch is
    reported after a minute instead of ten seconds.

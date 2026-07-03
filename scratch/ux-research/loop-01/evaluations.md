# Loop 01 — Evaluations

Each persona runs the target behavior — *find the one wave that needs me, open
it* — on each candidate. Grounded likes/dislikes, one-line verdict.

---

## Sol (solo, 1 repo, 1–3 waves)

**× A — Attention Queue First**
- Likes: With one repo the header repo-filter is ignorable, and the "Needs You"
  band is exactly Sol's question ("is my wave stuck?") rendered as a band. When
  nothing's wrong it collapses to one green line — zero noise.
- Dislikes: The RUNNING/IDLE sections are ceremony for someone with three waves;
  the sectioning implies a portfolio Sol doesn't have.
- Verdict: Works. The band answers Sol's one question; sections are harmless
  overhead. **Pass.**

**× B — Enriched Repo List**
- Likes: The reason line (`Failed at gate · +210 −38`) is the upgrade Sol
  actually needed over today's bare "Failed."
- Dislikes: The repo sidebar is pure dead space for a one-repo user — a whole
  column naming the single repo Sol already knows they're in.
- Verdict: The row is right; the sidebar is a tax. **Pass, grudgingly.**

**× C — Portfolio Board**
- Likes: Nothing specific to Sol's scale.
- Dislikes: Three columns for three waves is mostly empty air; the board format
  screams "you have a portfolio" at someone who has a project.
- Verdict: Over-built for Sol. **Weak pass / mild fail.**

**× D — Command-Palette**
- Likes: `⌘K ↵` straight into the wave's terminal is genuinely the fastest
  unstick Sol could ask for.
- Dislikes: With 1–3 waves, Sol will likely just look at whatever's on screen;
  the palette's value (skipping navigation) is small when there's nothing to
  navigate.
- Verdict: Nice accelerator, not load-bearing at this scale. **Pass as add-on.**

## Tess (team lead, 6 repos, 15–30 waves)

**× A — Attention Queue First**
- Likes: The counted "NEEDS YOU (2)" band is her 20-second triage in one glance,
  across all repos at once — no repo-at-a-time scanning. Reasons let her decide
  *without opening* ("PR limit — that's a 10-second land, not a real fire").
- Dislikes: She lost the standing repo list; she can't see "payments" as a place
  and has to use the filter menu to answer "how's payments specifically."
- Verdict: Best triage of the four for her default question. **Strong pass.**

**× B — Enriched Repo List**
- Likes: Attention-sorted so fires float up even on `.all`; the "N need you"
  rollup in the header is a real overview number.
- Dislikes: It's still a single scrolling list — a third fire appearing while
  she's mid-scroll is easy to miss vs. a counted band. The sidebar tempts a
  repo-at-a-time habit that's slower than seeing everything.
- Verdict: Better than today, but overview is emergent not asserted. **Pass.**

**× C — Portfolio Board**
- Likes: This is Tess's dream glance — "is anything wrong" is answered by whether
  the left column has cards, *pre-attentive*, before she reads a word. Repo-color
  chips let her see "payments is the red cluster."
- Dislikes: At 30 waves the IDLE column is a wall she has to visually mute; needs
  good collapse. Can't group by repo *and* status simultaneously.
- Verdict: Fastest "is anything on fire" of all four. **Strong pass.**

**× D — Command-Palette**
- Likes: Fast to *open* a known target.
- Dislikes: Gives her no ambient portfolio health — the app on her second monitor
  shows nothing until she invokes it. Triage is her job; a hidden palette hides
  exactly what she needs standing.
- Verdict: Wrong tool for a watcher. **Fail as primary; fine as accessory.**

## Kai (power user, terminal-first, keyboard)

**× A — Attention Queue First**
- Likes: Sectioned + `⌘↵` on the top row is a decent keyboard path; reasons
  inline mean he doesn't open a wave to learn it's just PR-limited.
- Dislikes: It's still a mouse-shaped surface he has to look at; the band is
  chrome between him and the terminal.
- Verdict: Tolerable, not his. **Pass.**

**× B — Enriched Repo List**
- Likes: Least GUI ambition; closest to a plain list he can arrow through.
- Dislikes: Sidebar + rows is still point-and-click by default; the chevron
  affordance is a mouse target, not a keystroke.
- Verdict: Inoffensive but slow for him. **Weak pass.**

**× C — Portfolio Board**
- Likes: Nothing — spatial cards are a mouse structure.
- Dislikes: Two-axis navigation (columns × cards) is the *most* pointer-bound of
  the four; actively fights keyboard flow.
- Verdict: The GUI reimplementation he distrusts. **Fail.**

**× D — Command-Palette**
- Likes: This is *his* interaction. `⌘K`, preselected fire, `↵` into the
  terminal harness pane, zero pointer — and it leans all the way into "frame,
  don't render." Faster than his shell alias.
- Dislikes: Wants to confirm `↵` lands in the *terminal*, not a rendered detail;
  if it opens a GUI panel he's out.
- Verdict: The only candidate that beats Terminal.app for him. **Strong pass.**

## Maya (onboarding, 1 repo, 4–5 waves)

**× A — Attention Queue First**
- Likes: The band's label *plus* reason ("Waiting — 3/3 PRs open, land one to
  continue") teaches her the model while triaging — the copy does double duty.
- Dislikes: Three status sections introduce vocabulary (idle vs running vs
  waiting) with no explanation of the difference beyond color.
- Verdict: The reason copy is a great teacher. **Pass.**

**× B — Enriched Repo List**
- Likes: Labeled rows, explicit chevron affordance — she can *see* that a row is
  openable. Highest hand-holding.
- Dislikes: Attention-sort is invisible logic to her; she doesn't know *why*
  order changed, so it can feel unstable.
- Verdict: Most legible affordances of the four. **Strong pass.**

**× C — Portfolio Board**
- Likes: Column headers name the states, which is itself a lesson.
- Dislikes: A card silently moving columns as status changes is disorienting when
  you don't yet own the model; density can overwhelm.
- Verdict: Teaches states by name but can bewilder. **Mixed pass.**

**× D — Command-Palette**
- Likes: Little — she doesn't know `⌘K` exists.
- Dislikes: Zero discoverability; a hidden keystroke is the opposite of a teacher.
- Verdict: Invisible to a newcomer. **Fail as primary.**

---

## Cross-cutting

### Where personas agreed
- **Bare status words fail; reasons pass.** Every persona's "like" on A/B/C
  cited the *reason* line (`Failed at gate`, `3/3 PRs open`), and today's flat
  `statusText` was implicitly the thing being fixed. This is unanimous.
- **Attention must be ranked, not insertion-ordered.** No persona wanted the
  registry-order list; all four rewarded floating fires to the top (band, sort,
  or column).
- **"Nothing needs me" must be glanceable in ~1s.** A's collapsed green line,
  C's empty left column, and D's green top row all won points for making *calm*
  legible, not just *alarm*.

### Where personas split (the real tensions)
1. **Glanceable dashboard vs. keyboard-first zero-UI.** Tess and Maya are best
   served by an *ambient, visible* surface (C's board, A's band) that shows
   health while the app just sits there. Kai is best served by an *invisible,
   invoked* path (D's palette) that never makes him look at a GUI. These pull in
   opposite directions: one wants more standing pixels, the other wants none.
   **This is the sharpest tension of the loop.** They may not be either/or — D
   composes over A/B/C as an overlay — but the *default* surface can only
   optimize for one, and that choice trades Kai against Tess/Maya.

2. **Does the repo sidebar (the shipped shape) survive?** Sol finds it dead
   weight; Tess wants repo-as-place but gets better triage when repo is demoted
   to a filter; A/C both remove it as primary and score *better* on the target
   behavior. The just-shipped "sidebar filters a list" hierarchy is challenged by
   every candidate that beats B. Keeping it (B) is safest to build but weakest on
   the actual behavior.

3. **Space vs. list for the portfolio.** C's board wins Tess's pre-attentive
   glance but loses Sol (empty columns) and Kai (pointer-bound). A's sectioned
   list is the compromise that nobody rates best but nobody fails.

### Per-candidate standing
- **A (Attention Queue):** the consensus non-loser — passes all four personas,
  is nobody's favorite except arguably Tess-adjacent. Safest *rethink* of the
  surface. Carries Sol, Tess, Maya solidly; Kai tolerates it.
- **B (Enriched List):** cheapest, most legible affordances (wins Maya), but its
  overview is emergent and its sidebar taxes Sol. The "do the minimum" option.
- **C (Portfolio Board):** highest ceiling for Tess (pre-attentive triage),
  hard floor for Sol and Kai. A team-lead specialist.
- **D (Command-Palette):** the only thing that wins Kai and beats his shell, and
  the only outright *fail* for Tess and Maya as a primary. Clearly an *overlay*
  that should ride on top of A/B/C, not the base surface.

**The decision a human should make:** pick the *default* surface between A (band)
and C (board) — the safe-consensus vs. the team-lead-optimized — knowing that D
(palette) should be built as an overlay on whichever wins, to keep Kai. B is the
fallback if we want the smallest possible change to the shipped code. This loop
deliberately does not pick; loop 02 should gather the missing evidence (see
questions.md).

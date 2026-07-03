# Concerto personas

Durable across loops. These are the people we simulate using Concerto. Each is
defined by scale (how many waves/repos), what they're trying to do, and their
tolerance for UI chrome — not demographics. Update when a loop teaches us a
persona is wrong or missing.

---

## Sol — the solo indie dev
**Scale:** 1 repo, 1–3 waves at a time.
**Context:** Building their own product. Concerto runs one or two waves while
they do other work; they check in a few times an hour.
**Wants:** "Is my wave moving or stuck?" One-glance answer. If stuck, get into
it fast and unstick it.
**Tolerance for chrome:** Low. A repo sidebar with one repo in it is pure
overhead to Sol. Anything that isn't their wave's state is noise.
**Fails when:** the UI makes them hunt across panels for a single wave's status,
or spends screen space on portfolio machinery they don't have.

## Tess — the team lead
**Scale:** 6 repos, 15–30 waves, several waiting/failed at any moment.
**Context:** Watches the whole portfolio between meetings. Glances for 20
seconds, needs to know where to spend attention, then leaves.
**Wants:** "Across everything, what is blocked or broken *right now*, and in
which repo?" Triage, not depth. Repo grouping matters to her — she thinks in
"the payments repo is on fire."
**Tolerance for chrome:** Medium — she'll accept density if it buys her
overview. Hates having to click into each wave to learn it's fine.
**Fails when:** attention-needing waves are visually equal to healthy ones, or
she has to filter to one repo at a time to see the whole board.

## Kai — the loopflow power user
**Scale:** 2–4 repos, lives in `lf goal` loops and the terminal all day.
**Context:** Runs waves from the CLI; Concerto competes with Terminal.app for
his daily driving. Deeply bought into "frame, don't render" — he wants the
vendor TUI, not a GUI reimplementation of it.
**Wants:** Get to a wave's *terminal* in one keystroke. The GUI earns its place
only if it's faster than his shell and doesn't hide the terminal.
**Tolerance for chrome:** Very low, but for a specific reason — he distrusts UI
that mediates the terminal. Keyboard-first or it's dead to him. ⌘K over mouse.
**Fails when:** the fastest path to a wave's terminal runs through pointing and
clicking, or the app renders wave state it should have just framed.

## Maya — the onboarding dev
**Scale:** 1 repo, 4–5 waves, two weeks into using loopflow.
**Context:** Still learning what a wave *is* and what its states mean. The UI is
her teacher.
**Wants:** The screen to tell her not just the status but what to *do* about it.
"Waiting" means nothing to her yet; "Waiting — 3 of 3 PRs open, land one to
continue" teaches her the model.
**Tolerance for chrome:** High — she wants labels, affordances, explanation. The
density that helps Tess can overwhelm Maya if it's unlabeled.
**Fails when:** status is a bare word with no reason, or the affordance to act
on a wave is invisible/implicit.

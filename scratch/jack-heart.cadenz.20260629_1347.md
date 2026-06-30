# Cadenza Launchboard — design

## Dream

Make Concerto (the loopflow desktop app) ruthlessly optimized for one job:
building Cadenza. Today Concerto is the generic multi-wave / governance
surface. This redesign narrows it into a **dashboard + launchboard** for a
single project — the place Jack opens to start, continue, and ship Cadenza work.

> "I want to make the Desktop app ruthlessly optimized for the experience of
> building Cadenza specifically."

Three work-shapes the launchboard must handle:

1. **One-off fix** → one PR, fully deployed (incl. release).
2. **Start a project** → `lf design --ide` → kickoff → wave.
3. **Continue the next** → ingest the next item on any project and build.

Backing object for a unit of work = an Asana task. Cadenza is the place to get
opinionated about the design tradeoffs the generic model leaves open.

### Goal context

- Primary: drive adoption in Jack's own daily music practice.
- Next: onboard his son.
- Overhang: product is more capable than current use; the "killer feature" is
  still moving — expect the daily-practice area to churn.
- Long-term tech depth: "understanding" the music — an interface built around
  LLM integration + MusicXML. Pays off in adoption later, needs investment now.

Constraint: areas/projects must be **cheap to rename and split** over time.

## What I found (grounding)

Two repos, two surfaces:

- **loopflow** (this repo, this branch) — Concerto / the launchboard. The *tool*.
- **cadenza** (`~/src/cadenza`) — SwiftUI app + FastAPI server, "music practice
  app for teachers and students." The *content*. Existing waves: `progress`,
  `score`, `video` — detailed, mature plans.

Existing Cadenza waves are cut by feature surface (progress / score / video).
The new cut: **Core** as trunk, with **Scores** and **Feedback** as sub-projects
(see Decisions). The earlier "LLMs" area folded into Feedback as its tech stack.

## Method (Jack's plan)

1. Decide a **starting roadmap** to use as the test launching area.
2. **Dogfood** it — drive Cadenza work through Concerto and find where the
   loopflow infra + UI hold it back. That friction defines the launchboard work.

## Decisions

- **This session = roadmap first**, in `~/src/cadenza`. Launchboard comes after,
  designed against a real roadmap and the friction of dogfooding it.
- **Core is the trunk; two sub-projects, not three.** LLMs folds *into* Feedback
  as its tech stack — the intelligence isn't a sibling, it's how Feedback works.
  - `Core` — make daily practice a habit I (and my son) keep   [progress: momentum, history]
    - `Scores` — work on the written page (mark up, isolate, manipulate)
    - `Feedback` — understand my practice and respond; LLM / musical-language /
      multimodal stack lives here   [video]
  - Existing waves: `progress` → Core; `video` → Feedback; `score`
    reading/annotation → Scores; `score` MusicXML + notation-intelligence → Feedback's stack.
  - Daily-practice & son-onboarding aren't areas — they're the adoption spine.
  - Launchboard consequence: **projects nest** — a trunk with sub-projects.

- **Two postures — and they're the two work-shapes the launchboard must handle.**
  - `Scores` = **product**. Decide the most important UXes and ship them. Drives
    daily-practice adoption now. (Annotate, isolate, manipulate — partly built.)
  - `Feedback` = **research + data collection**. Build and experiment with the
    tech stack; use Cadenza's UI to easily collect data from Jack's own practice.
    **Not** trying to make it work in-product yet. Long-running, exploratory.
  - This is the duality from the kickoff: one-off shippable increments (Scores)
    vs a long-running project (Feedback). Cadenza tests both shapes at once.
- **Frame around behaviors to drive, not cartography of the solution.** A roadmap
  item is "someone can now do X," ordered by which X earns daily use.

## Behaviors to drive

Spine: **me first, my son next.** Every behavior is judged by "does this make
someone open Cadenza tomorrow."

**Core — the daily loop becomes a habit**
- I open Cadenza and know exactly what to practice today.
- I finish and can see I'm building something (history I trust, not gamified noise).
- My son sits down and practices without me hovering.

**Scores — I mark up and reshape the music** (perform/pedal is shipped baseline, not the edge)
- I annotate a score — fingerings, reminders, a teacher's marks layered over mine — persisted and synced.
- I reshape how a piece reads — crop, reorder, pull passages out as loop-able practice regions.
- I change the music itself — edit, transpose, simplify a piece down for my son.

**Feedback — understand my practice and respond** *(research track: build the stack
and collect data; not a product feature yet)*
- While I play, I say in words how it feels / what I'm struggling with — hands-free.
- Cadenza captures the take: audio + video + my voice notes + score context.
- That data feeds an experimental stack — musical-language understanding,
  multimodal LLM feedback — built and tested offline.
- North star (later, when the stack earns it): named struggle → targeted exercise
  → daily loop. **For now: collect good data from my own practice.**

## Lead behavior: turn a passage into an exercise

The first behavior to drive, and the through-line that unifies all three areas:

> Highlight 4 bars of a real score → turn it into musical language →
> apply a musical transformation → get an exercise I can practice.

Jack already does the input by hand: marks chords, fingerings, shifts. The
unlock is everything downstream of "isolate a passage." This is the resolution
of the overhang — the app stops storing music and starts working on it.

**Pipeline** — splits across the two postures:

1. **Mark it up, fast** — chords, fingerings, shifts, by hand. (**Scores** —
   product, partly exists.)
2. **Highlight & isolate** — select a passage (e.g. 4 bars). (**Scores** —
   product; near practice-regions.)
3. **Turn it into musical language** — selected region → structured notation
   (MusicXML). *The keystone.* (**Feedback** stack — research.)
4. **Apply a transformation → exercise** — inversion, harmony, rhythmic changes,
   bow patterns, shift strategies. (**Feedback** stack — research.)
5. **Practice it** — renders, playable, enters the daily loop. (**Core**.)

Steps 1–2 are shippable Scores product now; 3–5 are Feedback's research north
star, not near-term shippable.

**Two flavors of transformation:**
- *Note-changing* (inversion, reharmonization, rhythmic variation) → a new phrase.
  Deterministic theory — `music21` on the FastAPI server.
- *Practice-strategy* (bow patterns, shift strategies, fingerings) → same notes,
  new practice instruction. Pedagogical judgment — where the LLM earns its place.

**Keystone risk:** step 3 trust. Wrong transcription → garbage exercise. Scoping
lever: Jack plays strings — single-line / double-stops make 4-bar transcription
tractable. Start monophonic, human-in-the-loop confirm.

## Feedback v1 — the capture instrument

Goal: make Cadenza a great instrument for collecting Jack's own practice data.
Record everything, experiment offline, don't ship feedback-as-feature yet.

**Nested units (auto-anchored to the music):**
- **Segment** = one routine step. Recording starts on practice-session launch and
  **restarts every time you move through the routine** (scheduled or user-controlled).
  ~5–10 min A/V by default, anchored to its routine item for free — no manual tagging.
  *The routine data model is the label.*
- **Clip** = a short excerpt marked for feedback. Easy to make. Max clip length
  bounds what's sent to the LLM.

**Record everything, send clips.** Full segments → research corpus. Clips → the
bounded thing the model sees.

**Experiment dials (config-driven — experimentation is the point):**
- modality: video-only / audio-only / both
- size/duration: segment length, clip cap, resolution / bitrate
- the question they answer: what makes the LLM problem workable to start?

**Foundations:** `video` already persists `VideoSubmission` → S3. The corpus needs
a home Jack can pull from for offline experimentation (export / server endpoints),
not just in-app playback.

## Decision — clips are made in practice review

- **Clips are made after the fact in practice review**, not live. During play,
  Cadenza only records (segmented by routine step). Clipping is a **practice-review**
  surface: scrub a segment, pull excerpts.
- Two-for-one: review-time clipping is a real **self-review** behavior on day one
  *and* the mechanism that generates clipped data for the research corpus. The
  self-review loop is usable before any AI exists ("starts with self").
- So **Feedback v1 = capture instrument + review surface.** AI is downstream of both.

## Decision — roadmap representation

**Flat waves** (`core` / `scores` / `feedback`) with the trunk/sub-project
hierarchy documented in each README. Tooling-safe, trivially re-cuttable; the
launchboard renders the tree from metadata later.

## Next action — flush to cadenza (delegated to a subagent)

In a cadenza worktree created with `lf op wt create`, deployed via `lf op land`:
- Recut `progress` / `score` / `video` → `core` / `scores` / `feedback`, carrying
  existing item content forward where it fits.
- `core` — practice-session spine + momentum/history (from `progress`).
- `scores` — product UXes: annotate, isolate, manipulate. Behavior-framed items.
- `feedback` — research + data collection. First item = **capture instrument +
  practice-review surface** (segmented A/V recording, review-time clipping, config
  dials, corpus export). Notation / musical-language work is feedback's stack.
- Each README documents the Core-trunk hierarchy and posture (product vs research).

## Parked

- Trusted path INTO musical language (deep path, later): MusicXML-only vs
  OMR/LLM transcription + confirm vs assisted manual entry.
- Backing-object specifics: does Asana-task-as-unit hold for the research track
  and nested projects? (Bleeds into loopflow — see launchboard implications.)

# The Living Website

Screenshots of loopflow, taken by loopflow, from loopflow building loopflow —
published automatically and never stale. The website stops being a brochure
that rots and becomes another surface the system keeps alive.

## Revision 2 (2026-07-17): one good update now, a base to build on

Jack's direction supersedes the install-hook model below: "remove all the
install.py stuff and just make a simple script for capturing screenshots and
placing them in the local branch's website with whatever Loopflow is
currently installed … later we can add something to install the latest
loopflow and run the whole process and then automate that … for now I just
want to update the website once in a way that will be good to build on."

Implementation reset — replace the current machinery with:

1. **One script, `scripts/capture_screenshots.py`** (replacing
   `refresh_website_screens.py`). Reads the `website:` section of
   `scripts/screenshots.yaml`, launches the installed app
   (`/Applications/Loopflow.app`, `--executable` to override) once per view,
   and writes each PNG plus one sidecar
   (`{captured_at, wave, app_version}`) directly into `website/static/` in
   the current worktree. No `--publish`, no branch guard, no staging beyond
   a temp file, no perceptual diff — `git status` shows what changed and a
   human commits it like any other change.
2. **Provenance softens to what the installed app can say.**
   `app_version` comes from `CFBundleShortVersionString`; the
   `LoopflowSourceCommit`/`LoopflowSourceDirty` stamping and all other
   install.py changes (the post-promote hook, `--no-screens`) are removed —
   restore install.py to its pre-feature shape. `app_commit` leaves the
   sidecar.
3. **Liveness informs, never blocks.** The script asks `lf status <wave>
   --json`; whatever it learns (served, unserved, no registry state) is
   printed and the capture proceeds. The human looks at the PNGs before
   committing. `require_live_wave` and the `.status.json` sidecar go away.
4. **Homepage renders what exists.** A figure renders when its image
   exists; the provenance caption line appears only when the sidecar exists
   and parses. No triple requirement.
5. **Deploy gate shrinks to visibility.** `check_website_screens.py`: an
   image without a parseable sidecar is an error; a stale `captured_at`
   (>14 days) is a warning; exit 0 on warnings.
6. **Swift stays.** The view knob, width/height/appearance envs, snapshot
   path, and the width-pin fix are the durable base — untouched.
7. **Tests shrink to the surviving surface**: manifest completeness,
   capture environment, sidecar round-trip, gate behavior, caption
   rendering (sidecar-optional).

The ladder back up, explicitly later: install-latest-loopflow +
run-the-whole-process as one command, then automation (hook or schedule),
then the Done Whens below.

## Revision (2026-07-17): prove the process, don't chase freshness

The first implementation landed (capture module, install hook, deploy gate,
Swift view knob, showcase rendering). Review against this doc plus Jack's
direction resets the bar. **The goal this round: one honest end-to-end run
that works and is replicable.** Always-fresh-per-install and
per-main-commit reliability are explicitly deferred — Done When 2 (four
unattended weeks) and the cron backstop are parked, not active.

What must change in the implementation:

1. **Liveness bar: served wave only.** `require_live_wave` currently refuses
   when no Task process is alive. Live state is currently imperfect and that
   is fine — red/failed states are honest and publishable ("red dots ok").
   Require that the wave is real and served; delete the no-live-task
   refusal and its test. Update the module docstring's claim to match.
2. **Deploy gate: structural failures block, age warns.** The 14-day age
   check currently fails the deploy, which couples unrelated docs/website
   shipping to app promotion. Split `validate_capture` into structural
   errors (missing/invalid sidecars, wrong wave, non-2x size, unserved
   status snapshot — still fail) and freshness (stale `captured_at` — print
   a loud warning in `check_website_screens.py`, exit 0). Keep the
   future-dated check as a structural error. Adjust tests to pin both
   behaviors.
3. **Fix the hollow width legs (review finding, confirmed).**
   `LOOPFLOW_UI_TEST_WIDTH` is now applied only inside `uiTestSnapshot`,
   which requires `LOOPFLOW_UI_TEST_SNAPSHOT_PATH` — so
   `WaveSurfaceStateTests`' 900/1440 legs both render at the default window
   size and prove nothing about width. Pin the window width whenever the
   env var is set even without a snapshot path (restore a view-level width
   pin gated on "no snapshot path"; keep snapshot-time `setContentSize`
   for capture runs, which also set height).
4. **Not a product surface.** The `lf website-screens` skill/flow are gone
   (already removed) and stay gone: this is repo-internal process, invoked
   by the install hook or by hand —
   `uv run python scripts/refresh_website_screens.py --publish`. The
   scripts and their docstrings are the documentation; no repo skill, no
   docs page.
5. **scratch/questions.md** — resolve the answered items: straight-to-main
   publication stands; the freshness/churn tension is resolved by (2); the
   busted-live-state note is superseded by (1); the backstop question is
   parked with the revised bar.

Acceptance for this round: `python/tests/test_website_screens.py` and the
website suite green; `swift build` compiles; then a human-driven capture run
on the laptop (promote a provenance-stamped build, serve the product wave,
run refresh with `--publish`) produces committed captures — red dots and
all — and rerunning it a second time reports "no perceptual changes" or a
clean update. The Done Whens below are the horizon, not this round's bar.

## Problem

The site has no product imagery: every prior asset was Maestro/Concerto-era
and got deleted rather than shown stale. That rot was structural — captures
were manual, so they aged the moment the UI moved. Meanwhile the most
compelling demo we own is free: the real product/infrastructure/intelligence
waves running in this repo every day. Nothing connects the two.

## Demo

On the laptop, the same command that already ships a new build:

```bash
uv run python scripts/install.py local --use
# ... builds lf + Loopflow.app, promotes it, then:
# website-screens: captured context-lab.png (changed) wave-surface.png (unchanged)
# website-screens: committed 1 capture from build 0.11.4-dev
```

Then open loopflow.studio: the homepage shows the Context Lab over the real
product wave — actual task names, actual receipts — captured from the build
now running on the laptop, with a caption stating when and from what. No
human took a screenshot; installing the app *was* the screenshot run.

## Done Whens

Proof under duration, not capability checkboxes:

1. **Live subjects only.** Every product image on loopflow.studio is captured
   from this repo's own running waves (`-ui-test-mode live`, real registry) —
   zero mock-fixture or hand-staged captures published. The capture run
   fails, rather than falling back to fixtures, when live state is absent.
2. **Four unattended weeks.** For four consecutive weekly releases, every
   published screenshot was ≤7 days old at deploy time and captured from an
   app build no older than the one running on the laptop that week —
   produced and committed by the install hook (cron as backstop) with zero
   manual capture and zero human repair of the pipeline. A rescue resets the
   streak.
3. **Staleness is unshippable.** A referenced-but-missing image cannot render
   (holds today, keep the test); an image older than 14 days fails the
   deploy gate loudly instead of shipping silently. Both proven by tests, not
   convention.
4. **Provenance on every pixel.** Each published capture carries a sidecar
   (commit, wave, capture time) rendered as its caption — "captured
   2026-07-20 from the product wave" — and drillable in the repo to the
   `lf status --json` snapshot that was live at capture time.
5. **One knob, many views.** Context Lab, the Wave surface, and the
   machine-wide roadmap are all capturable through the same env knob with no
   per-view Swift work beyond the first; adding a fourth view to the site is
   a manifest entry, not an engineering task.
6. **No churn commits.** Captures that differ only in volatile pixels
   (timestamps, spinners) do not produce commits; over one quiet week the
   cron produces zero no-op commits while still catching a real UI change
   within one cycle. (Empirical: needs a perceptual-diff threshold tuned on
   real captures.)
7. **Beautiful, verifiably.** The site with imagery passes a rams
   visual/accessibility review with zero blocking findings; captures are
   2x-retina at a consistent 1440-width light appearance, framed by CSS (not
   baked pixels), and the homepage reads as one composed page on a 13"
   laptop and a phone.

## Design sketch (sketches, not decisions)

**Capture (Swift, the one product change).** A `LOOPFLOW_UI_TEST_VIEW`
launch knob: `context-lab` opens the wave-scoped Context Lab window
(`openWindow(id: "context-lab", value: .wave(...))` for
`LOOPFLOW_UI_TEST_SELECT_BRANCH`) before the existing `uiTestSnapshot()`
fires. Everything else — `SnapshotService`, `LOOPFLOW_UI_TEST_SNAPSHOT_PATH`,
`-ui-test-mode live`, the delay — already exists and is proven by
`scripts/prove_product_wave_surface.sh`.

**Manifest (`scripts/screenshots.yaml`).** A `website:` set — view, wave,
width, delay, output under `website/static/` — replacing the dead
`type: snapshot` branch. `generate_screenshots.py --set website` runs it.

**Refresh flow (`lf website-screens`).** An ops flow: validate live state
(`lf status product --json` has a served wave and ≥1 live task) → capture
set against the promoted app → perceptual-compare against committed images →
write sidecars → commit + push only real changes.

**Trigger: the install hook.** `scripts/install.py local --use` already
builds and promotes `Loopflow.app` on the laptop; it gains a post-promote
step that runs the refresh flow against the build it just shipped (skippable
with `--no-screens`, skipped automatically when live state is absent — an
install must never fail because a wave is down). So screenshot freshness
rides the existing dogfood rhythm: new build on the laptop → new captures on
the site. `lf cron add --flow website-screens --schedule daily` stays as the
backstop for weeks with no promotes. `website-deploy.yml` picks the push up
and deploys — that leg exists.

**Gates.** Deploy workflow gains an age check over `website/static/*.png`
sidecars (>14 days → fail). Website tests already enforce
present⇒resolves / absent⇒omitted; add sidecar⇒caption rendering.

**Frame (website).** Images ship as raw window captures; the site owns the
presentation — border, shadow, radius via CSS — so re-captures never bake in
styling and a design change never requires re-shooting.

## Declared vs empirical

Declared (grounded in what exists): SnapshotService capture path, live-mode
launch, cron→commit→deploy loop, conditional render + tests.

Empirical (must be proven on real runs): whether live Context Lab data
renders compellingly at 1440 (density, wave with rich context history);
volatile-pixel threshold that kills churn without masking real change;
whether captures on a machine with the app already open conflict with the
launch-snapshot path (second instance?); commit cadence noise in the repo
history.

## Non-goals

- No GitHub-runner generation: CI verifies, the live machine captures.
- No mock/staged data on the public site, even as fallback.
- No video/gif pipeline yet — stills first; the same knob extends later.
- No MCP anything.

## Open questions

1. Which three views ship first? (Working set: Context Lab, Wave surface on
   `product`, machine roadmap.)
2. Backstop cron on the laptop or mini-heart? The install hook makes the
   laptop primary; the mini is always-on but its display state/appearance
   needs checking before it captures anything public.
2a. Should the sidecar record the app build (version + commit) so Done When
   2's "no older than the laptop's build" is checkable mechanically? (Lean
   yes — it's one field.)
3. Are captures reviewed before publish (PR the commit) or trusted straight
   to main? Straight-to-main matches the posture; a PR matches the "content
   includes live Linear titles" caution.

# The Living Website

Screenshots of loopflow, taken by loopflow, from loopflow building loopflow —
published automatically and never stale. The website stops being a brochure
that rots and becomes another surface the system keeps alive.

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

# Website + Docs Redo — Design Review

## What was implemented

The website and public documentation now describe the current loopflow: one
CLI as the agent API, persistent Waves as the operating context, fleet
steering through `lf`, and local state instead of a central server. The root
README is the short landing page; the docs tree carries the detailed product
contract; the homepage tells the product story without duplicating the
reference.

Agent readers get first-class surfaces: raw Markdown at `/docs/<slug>.md`,
`Accept: text/markdown` negotiation, Markdown 404 suggestions, `/llms.txt`,
`/llms-full.txt`, `/sitemap.xml`, and a published
`skills/loopflow/SKILL.md`. The skill is tested against the bundled agent
contract so the two entry points cannot quietly diverge.

The Mac app and install flow can now produce website captures from live Wave
state. A manifest selects views, the app opens and sizes the requested window,
the refresh script validates a 2x capture plus provenance and status sidecars,
and perceptual comparison avoids churn. The deploy gate rejects missing,
malformed, or stale published captures. No capture is rendered until its full
triple exists.

## Key choices

- Kept FastHTML and made the repo docs tree its source instead of introducing
  another publishing stack.
- Made the homepage story-first and folded the former team/story route into
  the home page. Retired product names and dead media were deleted rather than
  preserved as compatibility content.
- Pinned tests to navigability, media integrity, machine-reader contracts, and
  accessibility—not mutable marketing copy or section counts.
- Published an installable skill rather than an MCP surface. Outside agents
  learn the same CLI-only operating contract as loopflow-launched agents.
- Required real local Wave state for product captures. Missing or invalid live
  state skips the optional install hook; it never falls back to fixtures.
- Kept capture presentation in CSS and raw pixels in PNGs. Reframing the site
  does not require recapturing the product.
- Allowed an unchanged status snapshot during publication when the image
  changed. The atomic unit is a complete image/provenance/status triple, not a
  demand that all three files have different bytes on every run.

## How it fits together

`docs/` is the written source. `website/main.py` renders that Markdown for
humans and exposes the same source directly to agents, then derives discovery
surfaces from the docs navigation. The root README routes readers into those
pages, while the published skill compresses the runtime contract for external
agents.

For imagery, `scripts/install.py local --use` promotes the app and invokes the
optional refresh. `scripts/refresh_website_screens.py` reads the capture
manifest, proves the selected Wave is live, drives the app's test-mode window
selection, validates and installs complete capture triples, and can publish
perceptible changes through `lf commit -p`. Website rendering and deploy checks
consume those same triples.

## Risks and bottlenecks

- The four-week unattended proof is intentionally not claimable from this PR.
  It needs four real release cycles, not more unit tests.
- The current local `product` Wave cannot be captured: its PM snapshot has a
  stale Project Session and the Wave is not served. The verified behavior is a
  clean `--skip-unavailable` exit, so this branch contains no current product
  image to review visually.
- The perceptual threshold is a declared starting point until real captures
  show whether it suppresses volatile pixels without masking meaningful UI
  changes.
- Freshness and zero-churn remain in tension for genuinely unchanged pixels.
  Sidecars do not lie about recapture time, so a capture can eventually fail
  the 14-day deploy gate rather than creating metadata-only commits.
- Direct-to-main publication assumes the capture host is on a clean default
  branch. The publisher refuses any mixed worktree or incomplete triple before
  calling the push-capable `lf commit` path.
- The full gate spends most of its time in
  `wave_resolution_matrix` (349.5s of the 553s total). This branch does not
  touch that matrix, but it is the dominant reviewer feedback delay.

## What's not included

- No mock, fixture, or hand-staged public screenshot.
- No automated backstop schedule; the capture host still needs to be chosen.
- No video or GIF pipeline.
- No MCP server or alternate agent SDK.
- No new static-site framework and no rewrite of maintainer READMEs under the
  Rust source tree.

## Done When status

| Proof | Gate result |
|---|---|
| Live subjects only | Enforced by the live-status preflight and `live` app mode; the unavailable local Wave correctly refused to capture. No real capture exists yet. |
| Four unattended weeks | Pending four release cycles by definition. |
| Staleness is unshippable | Missing/malformed/stale triple checks and referenced-image resolution tests pass. The current empty capture set also passes. |
| Provenance on every pixel | Triple validation and the rendered caption/status-snapshot link are covered and pass. Awaiting a real triple. |
| One knob, many views | Context Lab, Wave surface, and roadmap share the manifest and launch environment. Compilation passes; live execution remains unproven. |
| No churn commits | Unit proof ignores a volatile pixel and detects a meaningful region; the quiet-week empirical proof is pending. |
| Beautiful, verifiably | Axe, contrast, focus, mobile layout, and image-overflow tests pass. A RAMS visual review of live imagery cannot run until imagery exists. |

## Validation

- `uv run python scripts/test.py --all` — 6 suites passed in 553s: Python,
  Rust, website, Swift, end-to-end smoke, and Xcode build-for-testing.
- `uv run pytest python/tests/` — 81 passed.
- `cd website && uv run python dev.py test` — 63 passed, 3 skipped.
- `swift test --package-path swift -Xswiftc -gnone` — 229 passed.
- Xcode `build-for-testing` using the CI destination — succeeded.
- `cargo fmt --all -- --check` and
  `cargo clippy --all-targets -- -D warnings` — passed.
- `uv run python scripts/check_swift_multiplatform_boundaries.py` — passed.
- Ruff check and format check over every changed Python file — passed.
- `uv run python scripts/check_website_screens.py` — 0 published captures,
  therefore no stale or incomplete capture.
- `uv run python scripts/refresh_website_screens.py --skip-unavailable` —
  exercised the intended skip against the unavailable live `product` Wave.
- Hosted UI behavior was not run. The repository runner reserves that for
  `--ui-host`; this run had no rendering environment, and the required live
  `product` Wave was unavailable.

The full gate caught one stale compile-time reference to deleted
`docs/wave-authoring.md`; the Rust contract test now embeds the merged
`docs/waves.md` page instead. It also hardened exact Retina dimensions,
Markdown content negotiation caches, capture timeout/JSON errors, window
settling, and partial capture publication before the final run.

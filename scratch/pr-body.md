## Try it!

Run the full repository gate:

```bash
uv run python scripts/test.py --all
```

Exercise the agent-readable documentation locally:

```bash
cd website
uv run python dev.py serve

curl http://localhost:5001/docs/waves.md
curl -H 'Accept: text/markdown' http://localhost:5001/docs/waves
curl http://localhost:5001/llms.txt
curl http://localhost:5001/llms-full.txt
```

Check the live-image contract without needing a running Wave:

```bash
uv run python scripts/check_website_screens.py
uv run python scripts/refresh_website_screens.py --skip-unavailable
```

The current checkout reports zero published captures and cleanly skips refresh
because the local `product` Wave is not served. With live state and a promoted
app, the same refresh captures the manifest views, validates 2x dimensions and
sidecars, and installs only perceptible changes.

Validation at gate: 81 Python tests passed; 1,819 Rust tests passed with 2
skipped; 63 website tests passed with 3 skipped; 229 Swift package tests
passed; the Xcode build-for-testing, Ruff, Cargo fmt/clippy, and Swift
multiplatform boundary checks passed. The 6-suite repository matrix completed
in 553s. Hosted UI behavior remains the separate `--ui-host` gate and was not
run in this non-rendering environment.

## Intent

Replace the Maestro-era website and split documentation story with one current
product narrative and one Markdown source of truth. Humans get a concise
story-first site and task-oriented docs; agents get the same content as raw
Markdown plus a published operating skill. The product-image pipeline makes
future screenshots an output of running loopflow itself instead of a manual
asset that starts rotting immediately.

## Assumptions

- The CLI remains the only agent API; no MCP or SDK surface is planned.
- Published captures must come from this repo's live Wave state, never fixtures.
- Capture publication runs only from a clean default branch and may push via
  `lf commit -p`; the install hook is best-effort and never makes app promotion
  fail when live state is unavailable.
- A capture timestamp describes changed pixels. An unchanged image is not
  republished merely to reset the 14-day freshness clock.
- Linear, GitHub, per-machine SQLite/journals, and SSH Homes remain the product's
  ownership boundaries; the website should not imply a central loopflow server.

## Key decisions

- Keep FastHTML and render `docs/` directly.
- Generate agent discovery surfaces from the docs navigation and serve explicit
  Markdown variants with cache-correct `Vary: Accept` headers.
- Delete the waitlist, old product assets, Jekyll remnants, and obsolete story
  routes instead of maintaining two eras.
- Publish `skills/loopflow/SKILL.md` and test its core doctrine against the
  bundled `LOOPFLOW.md` contract.
- Capture raw app windows through one manifest-driven launch knob, then frame
  them in website CSS.
- Treat PNG + provenance + live-status snapshot as the publishable unit, with
  exact Retina sizing, age validation, atomic local installation, and
  perceptual no-op suppression.

## Not included

- Current public product imagery: the local `product` Wave has stale/unserved
  state, so no fixture image was substituted.
- The four-week unattended freshness proof or a tuned empirical diff threshold.
- A cron/launchd backstop; the always-on capture host remains undecided.
- Video, GIFs, MCP, a new static-site stack, or maintainer-doc rewrites.

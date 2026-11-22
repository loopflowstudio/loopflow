# dd8997: the website in the new faces, one bar, one palette, one ramp

## Outcome

A visitor on `/`, `/studio`, `/fantasia`, `/docs/*`, `/architecture/*` and
`/download` sees one site: Old Standard headlines (24px and up only, `fl`
apart, −3% tracking), Atkinson Hyperlegible Next for everything read or
pressed, Spline Sans Mono for commands. Every page is one frame: the same bar at the top
(the logo and "loopflow", For you, Docs, Company ▾, GitHub ↗, Install) and
the same footer at the bottom.
The docs read in the homepage's colors and type steps. The workflow card on
the homepage shows the step names the builtin Graphs actually run.

Reference: `DESIGN.md` ("The language", "What we avoid") and
`design/prototypes/website-fonts.*.patch` (both apply cleanly to this
checkout: `git apply --check -p0` passes).

## Source of truth

`:root` in `website/static/style.css` is the web token source (DESIGN.md
rule 4). After this change it holds every color, font, type step, space step,
radius and shadow the site uses; nothing below it writes a hex, `rgba()` or
`!important`.

## Decisions

**Fonts, self-hosted.** `website/static/fonts/` holds the latin subsets from
Google Fonts' CSS API (probed 2026-11-21; all OFL):

| File | Face | Axis |
|------|------|------|
| `old-standard-400.woff2` | Old Standard TT | 400 static |
| `atkinson-next.woff2`, `atkinson-next-italic.woff2` | Atkinson Hyperlegible Next | variable, 400–600 |
| `spline-sans-mono.woff2`, `spline-sans-mono-italic.woff2` | Spline Sans Mono | variable, 400–700 |

plus each family's `OFL.txt` (google/fonts repo). `@font-face` rules open
`style.css`, `font-display: swap`, absolute `url(/static/fonts/…)`. No
Old Standard 700 or italic: DESIGN.md names 400 only, and the patch's
request for 700/italic goes. The Google Fonts `Link` in `main.py` goes
(the `.main.patch` is superseded by deleting it). Arrows (→ ↗ ▾) are outside
the latin subset and fall back to the system face, as they do today.
`/static/{fname:path}` already serves subdirectories; Starlette's
`FileResponse` guesses `font/woff2`. Add a `Cache-Control` with a long
max-age for `/static/fonts/` only if it is one line.

**Portable architecture export.** `scripts/render_architecture_html.py`
inlines `style.css` and links Google Fonts. Drop the Google link and rewrite
`url(/static/` to `url({BASE_URL}/static/` in the inlined CSS, the same way
it already rewrites `href`/`src`. Cross-origin `@font-face` needs CORS, so
the static route answers font files with `Access-Control-Allow-Origin: *`.
Opened offline, the file falls back to Georgia/system/ui-monospace; that is
acceptable for a portable export.

**Serif discipline.** One rule sets serif text: `font-variant-ligatures:
no-common-ligatures; letter-spacing: -.03em` on every serif selector,
folded into the rules the patch appended overrides for (home h1/h2,
`.belief p`, `.tag span`, `.split h3`, `.appmock.duo h3`, docs h1,
`.install-option h2`). Anything serif under 24px becomes sans: `.appmock
.waves h4` (18px) → sans 600; `.docs-deck` (22px italic) → sans
`--t-intro`, upright; `.home-page .tag span` stays serif at `--t-name`
(30). The ramp moves to the patch's sizes: `--t-display: clamp(32px,
3.8vw, 54px)`, `--t-title: clamp(28px, 2.8vw, 38px)`, `--t-name: 30px`,
matching DESIGN.md's "up to 54 / up to 38". The mobile hero rule that drops
h1 to `--t-name` keeps it ≥ 24 (the `test_mobile` hero check).

**One frame.** A page on the site is one thing: skip link, bar, main,
footer. Today four functions build it four ways (`_studio_page`: bar, no
footer; `_product_page`: bar, homepage footer; `_docs_page` and `/download`:
`Navbar`, `SiteFooter`). They become one `SitePage(title, current, *main,
cls)` returning `Title`, `SkipLink`, then `Div(SiteHeader(current),
Main(*main, id="main-content"), SiteFooter(), cls=cls)`; extra heads
(`docs.js`, robots `Meta`, `COPY_SCRIPT`) are passed in `main` or appended
by the caller. `_studio_page` and `_product_page` collapse into it; docs
pass `cls="docs-page"`, download `cls="download-page"`, home and studio keep
`cls="home-page"` (their 132 scoped rules stay scoped; only the frame's
rules leave the scope).

`SiteHeader(current)` is today's `_topbar()` renamed; delete `Navbar()` and
its CSS (`nav`, `.nav-*`, `.external-link`, the backdrop-blur sticky bar).
`SiteFooter()` is today's homepage footer (its `text` and `links`); the
docs' `SiteFooter` ("Loopflow, a software instrument." / "Built by Loopflow
Studio", `.footer-*`, `.built-by`, global `footer` rules) is deleted, so
`/studio`, `/etude`, `/fantasia`, `/about` gain a footer and docs and
download show the homepage's. Choice made for review: the homepage footer
wins because its copy is already the shared one in `content.yaml`; nothing
is written new (`questions.md`). Header and footer CSS move from
`.home-page header`/`nav`/`footer` to `.site-header`/`.site-footer`,
unscoped, each inside a wrapper capped at the homepage's 1240px with its
48/16px padding, so both are pixel-identical on every page whatever the
content width between (docs run to 1440). The bar is static, as on the
homepage; the docs rail's `position: sticky; top: 100px` becomes `top:
var(--space-24)`. The docs bar's "Graphs" link goes with `Navbar` (the docs
sidebar and "For you" remain). Docs keep their own `Documentation` `nav`
landmark, so two labeled navs remain (the accessibility test checks
distinct labels).

**One install.** Install lives in three places: top-level `install`
(label, href `/#start`, which no element carries), `homepage.install`
(command, download, links) and the `/download` page, which hardcodes the
command string twice. `content.yaml` keeps one top-level `install` block
(label, `href: /download`, `install_command`, `links`, `download_url`);
`homepage.install` goes, `INSTALL_CONTENT = SITE_CONTENT["install"]`, and
`/download` reads `install_command` and `download_url` from it. The
homepage `footer` block moves to top level beside `navigation`, since every
page reads it. `/download` moves onto `.button solid` and loses its inline
`style=`.

**One palette.** `--d-*` maps onto `:root` roles and is deleted:

| Docs token | Becomes |
|------------|---------|
| `--d-paper`, `--d-surface` | `--bg` (docs lose their grey ground; homepage is white) |
| `--d-ink`, `--d-body` | `--text` |
| `--d-soft`, `--d-faint` | `--text-secondary` (`#85888d` failed 4.5:1 anyway) |
| `--d-rule` | `--border` |
| `--d-rule-2` | `--border-warm` |
| `--d-wine` | `--wine` |
| `--d-chip-bg`, `--d-chip-border` | `--bg-subtle`, `--border` |
| `--d-code-bg`, `--d-code-fg` | `--term-bg`, `--term-fg` |

Raw colors left after that (homepage tints, shadows, the drawings' avatar
colors, the stripe wash and stamp ground, the old nav's rgba, `#999`/`#f0f0f0`
in the old buttons, the docs table tints) each become a named `:root` token
(`--tint-chip-top`, `--shadow-card`, `--avatar-a`…) with its value unchanged,
or are deleted with the rule they belong to. The drawings keep their look.

**One ramp.** `--fs-*` maps onto `--t-*` and is deleted: display →
`--t-display`, deck → `--t-intro`, h2 (sans 600, 27) → `--t-head` (24),
sub 17 → `--t-lede`, body 15.5 → `--t-text`, ui 14 and small 13 →
`--t-small`, label 12 → `--t-meta`. Docs and download radii and spacing
snap to `--radius-*` and `--space-*` (6px → `--radius-sm`, 46px →
`--space-48` and so on). `--t-head`'s use in "Tokens today" becomes
"Subheading" (serif on the homepage, sans 600 in the docs).

**No `!important`.** The 17 exist to beat earlier rules: docs ones against
old global `p`/`a`/`ul` resets and their own earlier selectors; the homepage
ones (`.pair .halves`) against a later duplicate rule. Each is folded:
delete the losing duplicate, merge the pair into one rule. The homepage
tail (lines ~1393–1529) has many such pairs (`.appmock{padding-top}` twice,
`.works{padding-bottom}` three times, `.appmock .sketch` twice); merge them
while there.

**Mono only for the machine.** The docs' "Markdown source" link and the
architecture "Public user docs ↗" link become sans `--t-meta`, and the
`{ }` glyph prefix goes. That removes the website line from DESIGN.md's
"Monospace as decoration". The drawings' mono agent names stay (drawings
are left alone), so the pointer keeps that half.

**Workflow card.** `home.html`'s `.wfs` lines follow
`rust/loopflow-definitions/src/builtins/task/graph/{feature,code,research}.yaml`
as of 9bc0fdecca, keeping the card's existing grammar (`<b>` = a stop for
a person, `<span>` = runs on its own, `start`/`end` caps):

- `feature`: start · **explore** · plan → build → **human-review** · deliver → **verify** · end
- `code`: start · plan → build → **human-review** · deliver → **verify** · end
- `research`: start · research → **research** · end

The `<small>` lines and the `.cap` sentence stay word for word, including
"The default." under `feature` even though `code` is the fallback Graph
(noted in `questions.md`).

## Exclusions

The rotating headline word, both drawn app windows and their "Coming soon"
stamps, and all written copy (the one footer reuses existing copy; the
workflow card's `<small>` lines stay word for word). The internal
`/fonts`, `/design` and `/colors` pages (`internal_pages.py`) keep their
own Google Fonts links: they compare candidate faces by design. No dark
scheme: the site has none (`prefers-color-scheme` appears nowhere), so
"light and dark" means the OS in dark mode still shows the intended light
site with nothing inverted or unreadable.

## What goes

- `main.py`: the Google Fonts `Link`, `Navbar()`, the docs' `.nav-logo`
  brand, the old `SiteFooter()`, `_studio_page` and `_product_page` (into
  `SitePage`); `_topbar` renamed `SiteHeader`; the hardcoded install
  command on `/download`.
- `content.yaml`: `homepage.install` (merged into `install`).
- `style.css`: the old `nav`/`.nav-*`/`.external-link` block and its media
  queries, the global `footer`/`.footer-*`/`.built-by` rules, `--d-*`, `--fs-*`, the Instrument/Inter/JetBrains font stack,
  every `!important`, every duplicated homepage rule, every raw color below
  `:root`, the old `.btn` styles if the download page stops using them
  (it moves to `.button solid`).
- `scripts/render_architecture_html.py`: the Google Fonts link.
- `design/prototypes/website-fonts.css.patch` and `website-fonts.main.patch`:
  their screens ship, so they go (DESIGN.md: "Delete a prototype when its
  screens ship"), along with their mention in DESIGN.md's prototypes
  paragraph.
- `DESIGN.md`: the website lines under "Monospace as decoration" (docs
  links half), "Values outside the scale" (website docs/download and
  homepage raw colors and `!important`), "Two of the same thing" (two
  navigation bars; "The site sets Instrument Serif and Inter" becomes the
  apps-only line). "Tokens today": website type row → the three faces; the
  website ramp table → the shipped `--t-*` values; drop "The docs keep their
  own `--fs-*` ramp"; Color lists the tokens `:root` actually ships.
- Tests that name what is gone: `test_homepage.py:106` (JetBrains Mono →
  Spline Sans Mono), `test_navigation.py` (`.nav-logo` → `.site-header
  .brand`; `.home-page nav` selectors → `.site-header nav`),
  `test_mobile.py` brand/nav selectors.

## Acceptance check at gate

```bash
cd website && env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  uv run --no-sync python ../scripts/test_network.py \
  uv run --no-sync pytest tests/ -q < /dev/null          # all pass
uv run --no-sync python ../scripts/render_architecture_html.py --check
grep -c '!important' static/style.css                     # 0
awk '/^}/&&!d{d=1;next} d' static/style.css | grep -cE '#[0-9a-fA-F]{3,8}\b|rgba?\('   # 0
grep -rn 'googleapis\|gstatic' main.py ../scripts/render_architecture_html.py  # nothing
```

New e2e test (one file, `tests/e2e/test_faces.py`), each a thing a visitor
would notice breaking:

- On `/`, `/docs`, `/download`: `document.fonts.check('16px "Atkinson
  Hyperlegible Next"')` and the h1's computed family starts with
  `"Old Standard TT"`, after `document.fonts.ready`; no request leaves for
  `fonts.googleapis.com`/`fonts.gstatic.com` (Playwright request log).
- Every element whose computed family starts with Old Standard has computed
  font-size ≥ 24px and `font-variant-ligatures` containing
  `no-common-ligatures`, on `/`, `/studio`, `/docs`, `/download`.
- `.site-header` and `.site-footer` outerHTML are identical on `/`,
  `/studio`, `/docs` and `/download` apart from `aria-current`, and their
  bounding boxes' x and width match at 1440 and 390.
- The workflow card's step words for each Graph equal the steps parsed from
  the builtin YAML (the test reads the YAML, so the next Graph change fails
  it rather than silently drifting).

Then look at `/`, `/docs`, `/docs/orchestration`, `/download` at 1440 and
390 wide, light and OS-dark, in Chrome; screenshot each into `scratch/`.

## Slices

1. **Faces everywhere, self-hosted.** Fetch the five woff2 files and OFL
   texts, add `@font-face`, swap the font tokens, delete the Google `Link`,
   fold the patch's overrides into their rules (serif rule, ramp change, h4
   and deck to sans), update the architecture renderer and regenerate
   `docs/dev/index.html`, add `test_faces.py`'s first two tests. Shows
   whether Old Standard at the new ramp holds the homepage and docs h1
   without overflow at 390 (the riskiest visual unknown) and whether the
   portable export still renders. Delete the two patch files.
2. **One frame.** `SitePage` with `SiteHeader` and the homepage
   `SiteFooter` on every page, `.site-header`/`.site-footer` CSS, delete
   `Navbar`, the docs footer and their CSS, one `install` block with Install
   → `/download`, docs rail sticky offset, the download page onto
   `.button`. Update navigation tests; add the identical-frame test.
3. **One palette and ramp, no `!important`.** Map `--d-*`/`--fs-*`, snap
   docs/download radii and spacing to steps, tokenize the remaining raw
   colors, merge the homepage's duplicate rules, md-link to sans. Look at
   every page at both widths.
4. **Workflow card and DESIGN.md.** Card lines from the YAML plus its test;
   DESIGN.md pointers and "Tokens today" to what shipped.

## Docs written first (write-docs, 2026-11-21)

`DESIGN.md` already describes the shipped website: prototypes paragraph,
"What we avoid" website pointers, "Tokens today" type, ramp and color.
Slice 4's DESIGN.md work is done; check it against what lands and correct
the values (ramp clamps, token names) if the build moves them. No
user-facing docs change: the site's copy is excluded.

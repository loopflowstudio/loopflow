# Website

```bash
uv run python dev.py serve
uv run python dev.py test
```

Run from `website/`. Preview at `http://localhost:5001`.

Edit the homepage in `home.html`: the app mock, the workflows card, the command
card and the copy around them are one template, drawn by hand. `main.py` fills
in the download link and install command from `content.yaml`, which also holds
every other page's copy; `static/style.css` controls layout. `navigation` at the
top of the file is the one bar on the homepage and studio pages, including its
Company menu. Graphs navigation on docs pages goes to `/docs/orchestration`.

The `studio` block holds the company page at `/studio` and the `/etude`,
`/fantasia` and `/about` pages. An effort
page with no `points` renders its heading and one line.

The homepage's pictures are HTML, not captures. They show the intended product,
including commands and output that `lf` does not print yet.

Edit documentation in the repository's `docs/` directory. The dev commands
sync it into the website; `website/docs/` is generated.

## Testing

Browser and accessibility tests for `website/`. The dev helper syncs canonical
`docs/` into `website/docs/`, installs the Chromium browser, starts the app, and
runs the test suite.

Homepage touch targets must retain their minimum size with fallback fonts, whose
metrics differ across macOS and Linux. Run the website suite after the final CSS
or capture edit, including the fallback-font touch-target case.

```bash
cd website && uv run python dev.py test        # All website tests
cd website && uv run python dev.py test -a     # Accessibility tests only
cd website && uv run python dev.py sync-docs   # Refresh generated docs copy
```

After changing `docs/architecture.md` or its website rendering, regenerate the
portable HTML before gate and include it in the same change:

```bash
cd website
uv run python dev.py sync-docs
uv run python ../scripts/render_architecture_html.py
uv run python ../scripts/render_architecture_html.py --check
```

The website suite checks that `docs/architecture.html` matches the rendered
source. A Markdown-only edit can fail that check.

The introductions in `README.md` and `docs/index.md` share the same text. When
editing either introduction, update both and run
`uv run --project website --extra test pytest website/tests/test_readme_index_sync.py`.

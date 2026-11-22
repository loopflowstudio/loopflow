Read when the change touches the website (`website/`), its stylesheet or `docs/dev/index.html`.

- Serve on a spare port; `dev.py serve -k` kills whatever holds 5001: `cd website && PORT=5077 uv run python main.py`. Uvicorn reloads on edits under `website/` and sometimes exits; restart it.
- Drive it with the website venv's Playwright: `uv run --extra test python <script>` from `website/`. Keep the scripts outside the repo.
- To compare with main, serve the branch and route `**/static/style.css*` (the trailing `*` matters: the link carries `?v=<hash>`) to `git show main:website/static/style.css`.
- `docs/dev/index.html` is generated from the site: after any `style.css` edit, run `website/.venv/bin/python scripts/render_architecture_html.py` (the repo `uv run` lacks fasthtml); `tests/test_portable_architecture.py` fails until you do.
- To find broken links, crawl from the route list (`grep -n '@rt(' website/main.py`) and follow same-origin `a[href]`; about 84 pages answer.
- Focus rings: tab to the element, then wait ~400ms before reading or screenshotting. `.copy-btn` has `transition: all`, so an immediate read catches the outline mid-animation (white, or missing).
- Don't name a Playwright script after a stdlib module (`copy.py`): with the script's directory first on the path, it breaks the import.
- The homepage `/` has its own nav; the shared `Navbar` (with the `.btn-primary` Install button) is on `/download`, docs and studio pages.
- Ghostty config strings: `/Applications/Ghostty.app/Contents/MacOS/ghostty +validate-config --config-file=<file>` rejects unknown keys.

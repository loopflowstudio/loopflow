Read when the change touches the website (`website/`), its stylesheet or `docs/dev/index.html`.

- Serve on a spare port; `dev.py serve -k` kills whatever holds 5001: `cd website && PORT=5077 uv run python main.py`. Uvicorn reloads on edits under `website/` and sometimes exits; restart it.
- Drive it with the website venv's Playwright: `uv run --extra test python <script>` from `website/`. Keep the scripts outside the repo.
- To compare with main, serve the branch and route `**/static/style.css*` (the trailing `*` matters: the link carries `?v=<hash>`) to `git show main:website/static/style.css`.
- `docs/dev/index.html` is generated from the site: after any `style.css` edit, run `website/.venv/bin/python scripts/render_architecture_html.py` (the repo `uv run` lacks fasthtml); `tests/test_portable_architecture.py` fails until you do.
- `/design`, `/colors` and `/fonts` are standalone internal pages with inline styles; `style.css` doesn't reach them.
- Ghostty config strings: `/Applications/Ghostty.app/Contents/MacOS/ghostty +validate-config --config-file=<file>` rejects unknown keys.

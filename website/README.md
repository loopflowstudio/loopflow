# Website

```bash
uv run python dev.py serve
uv run python dev.py test
```

Run from `website/`. Preview at `http://localhost:5001`.

Edit homepage copy in `content.yaml`: the opening and its example loopflow,
the ownership strip, the product window, why Loopflow exists, the three
levels of autonomy, the parts and their guide links, and install.
`main.py` renders the sections; `static/style.css` controls their layout.
Features navigation points to `/#features` from every page.

The product window steps through real captures listed under
`homepage.showcase`: the Product Wave in `static/wave-surface.png`, an agent
session in `static/session-full.png`, and the merged pull request for the
highlighted task in `static/pr-result.png`. `static/home.js` animates the
step from the plan into a task; `pointer` and `highlight` locate that task's
row as percentages of the first image. A frame whose image is missing is left
out. Each Desktop image's `.json` sidecar records the capture date and app
version shown beneath the window.

Edit documentation in the repository's `docs/` directory. The dev commands
sync it into the website; `website/docs/` is generated.

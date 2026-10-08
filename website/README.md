# Website

```bash
uv run python dev.py serve
uv run python dev.py test
```

Run from `website/`. Preview at `http://localhost:5001`.

Edit homepage copy and examples in `content.yaml`. `main.py` renders the hero,
ownership strip, the For your agents and For you sections, and install;
`static/style.css` controls their layout. `navigation` at the top of the file
is the one bar on the homepage and studio pages, including its Company menu.
Flows navigation on docs pages returns to `/#flows`.

The `studio` block holds the company page at `/studio` and the `/etude`,
`/fantasia` and `/about` pages. An effort
page with no `points` renders its heading and one line.

The terminal section ships without a capture. The Mac capture is
`static/wave-surface.png`; its adjacent JSON file retains capture provenance,
and the image reserves its dimensions.

Edit documentation in the repository's `docs/` directory. The dev commands
sync it into the website; `website/docs/` is generated.

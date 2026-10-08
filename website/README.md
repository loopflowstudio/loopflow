# Website

```bash
uv run python dev.py serve
uv run pytest tests/
```

Run from `website/`. Preview at `http://localhost:5001`.

Edit homepage copy and examples in `content.yaml`. `main.py` renders the hero,
ownership strip, terminal Flow, organizing rows, Mac capture, purpose, and install;
`static/style.css` controls their layout. Homepage navigation points to each section;
Flows navigation on other pages returns to `/#flows`.

The terminal capture uses `static/cmux-flow.png`. Its space and reference remain
when the image has not yet been supplied. The Mac capture is `static/wave-surface.png`;
its adjacent JSON file retains capture provenance. Both images reserve their dimensions.

Edit documentation in the repository's `docs/` directory. The dev commands
sync it into the website; `website/docs/` is generated.

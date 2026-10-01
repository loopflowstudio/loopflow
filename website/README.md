# Website

```bash
uv run python dev.py serve
uv run python dev.py test
```

Run from `website/`. Preview at `http://localhost:5001`.

Edit homepage copy in `content.yaml`: the opening, model independence,
feature explanations and documentation links, and shared company practice.
`main.py` renders the sections; `static/style.css` controls their layout.
Features navigation points to `/#features` from every page.

Edit documentation in the repository's `docs/` directory. The dev commands
sync it into the website; `website/docs/` is generated.

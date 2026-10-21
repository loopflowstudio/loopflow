# Website

```bash
uv run python dev.py serve
uv run python dev.py test
```

Run from `website/`. Preview at `http://localhost:5001`.

Edit the homepage in `home.html`: the app mock, the workflows card, the command
card and the copy around them are one template, drawn by hand. `main.py` fills
in the download link and install command from `content.yaml`, which also holds
every other page's copy; `static/style.css` controls layout. The `cli` block is
the page for engineers at `/cli`: the commands and two plain lists, with no Mac
app. Backticks in its list copy set code. `navigation` at the top of the file
is the one bar on the homepage and studio pages, including its Company menu.
Graphs navigation on docs pages goes to `/cli#graphs`.

The `studio` block holds the company page at `/studio` and the `/etude`,
`/fantasia` and `/about` pages. An effort
page with no `points` renders its heading and one line.

The homepage's pictures are HTML, not captures. They show the intended product,
including commands and output that `lf` does not print yet.

Edit documentation in the repository's `docs/` directory. The dev commands
sync it into the website; `website/docs/` is generated.

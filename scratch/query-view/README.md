# Try the query-view study

```sh
uv run python -m http.server 8749 --bind 127.0.0.1 --directory scratch/query-view
```

Open http://127.0.0.1:8749/. Choose **Product + Needs you**, hide the first
Query-based views tile, and watch the later match take its place. Restore it
from the hidden tray. Choose **Product**, hide a tile, resize the remaining
split, then restore: the new ratio stays.

Enter a Task, leave a draft, add a sample shell and return. Focusing the shell
keeps the primary Session on the tile; zoom changes the face. Sample activity
starts/completes fixture Tasks so query membership changes. Move/resize first
to see new matches enter beside a retained arrangement.

The editor includes presets, live validation, pinning and a full split-tree map.
Queries and all conversation output are sample data. Nothing executes or sends.
State lasts until reload/reset; query syntax and layout density are provisional.
This is an interaction prototype, not production Desktop or lifecycle evidence.

```sh
node --test scratch/query-view/model.test.mjs
```

Five model checks cover defaults, overlaps, hidden edits, arrival reconciliation
and descendant scope. Browser checks also verified draft retention, focus/zoom
and invalid-query isolation. The implementation scope following this study is
[`../design.md`](../design.md).

# Try Dashboards in the Console

```sh
uv run python -m http.server 8749 --bind 127.0.0.1 --directory scratch/query-view
```

Open http://127.0.0.1:8749/?theme=dark. This rebuild uses LOO-489’s October 17
Console source: stable working Tasks, one titlebar, repository conversation,
Wave conversation + complete Task plan, and Task-only Shell/Files tabs.

- **Review desk** selects Available Tasks. Write in a tile, open its Task, add
  a Shell tab or edit a Files tab, then return. Drafts/tools belong to the Task.
- **Product watch** selects `(wave:product state:open) or state:available`. Hide LOO-489: its later
  occurrence becomes visible. Restore it from the Hidden strip. First visible wins.
- Drag tile headers to swap positions. Drag a divider, or focus it and use arrow
  keys, to resize. Hiding/restoring preserves edits to surviving splits.
- **+** beside Dashboards creates an independent Dashboard. **Edit Dashboard**
  edits only that Dashboard. Equal queries do not merge their identities/layouts.
  Returning to a prior query recovers its arrangement.
- Choose **Include an unstarted Task**, save, then open its brief. There is no
  Session, composer or Shell/Files toggle. Selecting it starts nothing.
- **Cmd-K** / the repository chevron opens Find Work for Tasks, Waves, Dashboards
  and repositories. Another repository opens a separate sample window. Repository
  scope is implicit; a `repo:` Dashboard query is rejected.

The study retains state in this browser’s local storage. Reset study clears only
its fixtures. Terminal output, Tasks and PRs are sample data; inputs send nothing,
shell commands do not execute, and document edits never write repository files.
HTML retention is not evidence of native terminal attachment or restoration.
There are no invisible named-query fixtures in the Dashboard editor.

`source-manifest.json` records the captured LOO-489 source; `dashboard.mjs` adds the
Dashboard interactions; `model.mjs` is the earlier disposable evaluator
(`node --test scratch/query-view/model.test.mjs scratch/query-view/model.test.cjs`).
October 17 model and browser checks: `f492be7f19:scratch/query-view/README.md`.

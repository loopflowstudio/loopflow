# Native Linear hierarchy — try-it + remaining live step

The implementation assumptions that were open questions are now decisions,
folded into `wave/infrastructure/MEMORY.md` (Planning-model section): Linear
Initiative/Project/Issue are authoritative; a project slug is derived
deterministically from the Linear Project name and duplicate slugs are a hard
drift error; `wave/<wave>/projects/*.md` is a read-only cache / migration seed;
KR state is a human/loop `[x]` judgment stored in Linear Project `content`;
Project definitions/KRs are edited in Linear then pulled with `lf pm sync`.

## Remaining live step (deliberate, not run in headless auto)

Every wave's `GOAL.md` still carries `pm.linear_project`. After redeploying `lf`:

```bash
lf pm init --wave infrastructure   # creates the Initiative, migrates labeled issues
lf pm show --wave infrastructure   # should now list native-project tasks as a table
```

- Assign exactly one recognized `project:<slug>` label to any straggler issue
  first — otherwise it's left behind and `pm.linear_project` is retained with an
  `unmigrated` count.
- Repeat for `intelligence` and `product`.
- Resolve the stranded `Datamodel` Linear project (no local wave points at it).

## Try-it (offline)

```bash
cargo test -p loopflow --lib ops::pm         # migration + duplicate-slug drift
cargo test -p loopflow --lib lfd::pm::linear # content/KR parsing, list_items mapping
```

---
requires: a change someone is about to read
produces: scratch/tour.yaml, the order to read the change in and what to skip
action_style: procedural
---
Write the reading order for this change, so whoever opens the diff starts where it makes sense.

**Read the change the way the reader will.** In a Task, `lf task changes <ID>`
lists the files and `lf task diff <ID>` shows them, uncommitted work included;
a diff it says is truncated continues one file at a time with
`lf task diff <ID> <path>`. Outside a Task, read `git diff` against the
branch's base. Read all of it before deciding anything.

**Order it for understanding.** Docs and the outcome first, then the data
model and public APIs, then the logic that uses them, then tests, generated
files last. Within that, put first what explains the change: the file that
makes the rest make sense.

**Hide only what is not worth opening:** a rename's fallout, regenerated
snapshots, a lock file. Never hide something a reviewer should judge.

**Write `scratch/tour.yaml` whole,** replacing what was there:

```yaml
at: <the full commit `git rev-parse HEAD` printed when you read the diff>
first:
  - path: <file, or folder ending in />
    why: <what it holds, in under ten words>
hide:
  - path: <file or folder>
    why: <why it is safe to skip>
```

Name only the files that need a place; the rest follow in the default order.
An entry without `why:` reads on under the one above it.

A tour is behind once a commit after its `at:` changes a file outside
`scratch/`. Committing the tour itself leaves it current.

Change nothing else. Your whole job is this file.

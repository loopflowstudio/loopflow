# 23217a open questions

- Stale window: a checkout with no Task or PR is removed after HEAD (its reflog, else the commit date) has not moved for 7 days. Jack said "old head" without a number; 7 days matches the old fixtures.
- `--all` keeps persistent checkouts (repo and Wave conversation checkouts carry `loopflow-persistent`), the default branch and the current checkout, beside unfinished Tasks and process-held paths.
- Not done: Jack's rule (relayed 2026-10-29 from the 23217a Task conversation) that a live but idle conversation shouldn't protect a dead worktree. Prune still treats any `lsof` path inside a checkout as "in use by a running process"; telling an idle agent conversation from live work needs Session availability per process. On 2026-10-29 this kept 9 manabot checkouts and 1 etude checkout. Proposed follow-up Task.
- Saved refs accumulate under `refs/lf/pruned/`; nothing expires them yet.

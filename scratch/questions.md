# Questions

- LOO-1113 adopted the concept review's recommendations headless (1a 2a 3a
  4a): every plan-reading command does the first read, a failed one retries
  after 60 s, the hub is read too, empty clones stay quiet on the server. All
  reversible; `scratch/plan.md` "Choices made for review" lists what was
  passed over.
- All `--subscribe` variants of plan-reading commands (`lf task list --subscribe`,
  `lf wave show --subscribe`, `lf wave status --subscribe`,
  `lf plan list --subscribe`, `lf repo tracker --subscribe`) skip
  `before_first_read` so every subscription reads the store alone. Reversible.

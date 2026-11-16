# LOO-925 questions

## Taken headless (reversible at review)

- **Any tracker, not Linear-only.** Linear and Asana stay as bridges; Notion is
  deleted. Basis in `tracker-mirror.md` ("Decision"): the 2026-11-02 parity
  decision recorded on `loo-691-combined`, Asana's rebuild in #1607, and the
  queue being required either way. If Linear-only is wanted, slice 4 deletes
  `asana/` instead of porting it and everything else stands.
- **Table name `tracker_effects`.** Alternatives: `planning_outbox`,
  `tracker_queue`.
- **Drain bound: 20 provider requests or 5 s per pass, concurrency 4.** The
  Linear allowance is 5,000/h shared by every lf process on a Machine (~11 live
  on 2026-11-05); 20 per pass keeps a pass well under it. Tune from
  `linear-requests.jsonl` after landing.
- **Migration marks copied old rows settled** so an older lf does not resend
  them; edits an older lf makes after install are delivered by that lf.
- **`lf task sweep` is deleted** with `ChapterSweep`: it sweeps Tasks outside
  the "current chapter", a rotation `planning.md` says no longer exists. If a
  sweep of Tasks outside the Wave's current Project is still wanted, it is a
  new command over Projects, not this one.
- **`~/.lf/chapter-locks/` keeps its name** until the contract Task: an older
  lf on the same store locks the same inode.

## Open

- Wire `linear_id` → a neutral name crosses Rust, Swift and ~10 DTO fixtures.
  Planned as a follow-up Task, not here. Is a neutral name wanted at all while
  Linear is the only tracker in use?

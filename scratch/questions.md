# Open questions (LOO-1106)

Reversible choices made with nobody here; each is in `scratch/plan.md` under
"Choices made for review" with what was passed over.

1. **Builtin `queue` Graph and `.lf/graphs/finish-queue.yaml` are deleted.**
   The Task said "update or remove `queue.yaml` after checking references":
   no Graph, Wave config or documented command uses either; tests and one
   `docs/surfaces.md` row name `queue`.

2. **The cached PR copy (`scratch/pr-title.txt`, `pr-body.md`,
   `.pr-copy-ref`) is deleted with `gate`.** Not named in the Task. Nothing
   in the repository writes those files; only `ops/pr.rs` readers and three
   tests remain.

3. **`.lf/graphs/pursue-auto.yaml` loses the outer loop `gate` decided**, not
   only the `gate` line.

4. **`lf scratch clear` keeps today's base** (the Task's recorded base, the
   parent's head when stacked); only delivery measures against the
   integration target.

5. **Wave `MEMORY.md` files that mention gate as a phase stay** (read-only in
   a Task checkout): `.lf/wave/intelligence/MEMORY.md:34,41,42`,
   `.lf/wave/infrastructure/MEMORY.md`, `.lf/wave/product/MEMORY.md:1012`.

6. **Carried from LOO-1104:** `.lf/wave/intelligence/MEMORY.md` (2026-09-28)
   still says `realign` owns memory curation; `wave-session` does since
   LOO-1104.

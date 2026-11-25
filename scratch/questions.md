# Open questions (LOO-1106)

Reversible choices made with nobody here; each is in `scratch/plan.md` under
"Choices made for review" and `scratch/concept-review.html` with what was
passed over.

1. **Single-owner delivery clear (no pre-sync clear when `resync == true`).**
   `prepare_pr` clears only when `resync == false`; `sync_for_delivery` keeps
   `scratch/` on disk during merge/conflict resolution, auto-drops pure
   stacked-scratch `UD` conflicts inside `start_owned_sync`, and clears once in
   `finish_sync` before verifying and pushing. Passed over: clearing before
   sync in `prepare_pr` and clearing again in `finish_sync`.

2. **Builtin `queue` Graph and `.lf/graphs/finish-queue.yaml` are deleted.**
   The Task said "update or remove `queue.yaml` after checking references":
   no Graph, Wave config or documented command uses either; tests and one
   `docs/surfaces.md` row name `queue`.

3. **The cached PR copy (`scratch/pr-title.txt`, `pr-body.md`,
   `.pr-copy-ref`) is deleted with `gate`.** Not named in the Task. Nothing
   in the repository writes those files; only `ops/pr.rs` readers and three
   tests remain.

4. **`.lf/graphs/pursue-auto.yaml` loses the outer loop `gate` decided**, not
   only the `gate` line.

5. **`lf scratch clear` stays as a standalone command with today's base**
   (the Task's recorded base, the parent's head when stacked); only delivery
   measures against the integration target.

6. **Wave `MEMORY.md` files that mention gate as a phase stay** (read-only in
   a Task checkout): `.lf/wave/intelligence/MEMORY.md:34,41,42`,
   `.lf/wave/infrastructure/MEMORY.md`, `.lf/wave/product/MEMORY.md:1012`.

7. **Carried from LOO-1104:** `.lf/wave/intelligence/MEMORY.md` (2026-09-28)
   still says `realign` owns memory curation; `wave-session` does since
   LOO-1104.

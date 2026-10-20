# Open questions / assumptions — LOO-525

- Assumption: `.lf/flows/pursue-auto.yaml` stays a superset of builtin pursue,
  so it gains `concept-review` + `loop: implement` after the first implement
  loop and before `gate`. Reversible: drop the two lines if pursue-auto should
  stay review-free.

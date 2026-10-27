# LOO-628 open questions and assumptions

All design questions settled by Jack Heart in review (2026-10-26); see the
design's "Concept review". Remaining, decided by the demo:
- Does Codex accept a copied rollout's encrypted reasoning under a different
  ChatGPT account? If not, Codex moves across accounts take the log fallback.
- A Session moved alone arrives on M with no Task attribution (the Task stays
  owned on S, and M may not know it). Simpler than carrying a `task_id` M
  cannot route; revisit with the Task move.
- On the default branch a Session moves only when clean and is never committed
  to or pushed; a repository primary on `main` with edits refuses to move.

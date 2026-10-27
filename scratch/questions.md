# LOO-628 open questions and assumptions

All design questions settled by Jack Heart in review (2026-10-26); see the
design's "Concept review". Remaining, decided by the demo:
- Does Codex accept a copied rollout's encrypted reasoning under a different
  ChatGPT account? If not, Codex moves across accounts take the log fallback.
- A Session moved alone arrives on M with no Task attribution (the Task stays
  owned on S, and M may not know it). Kept after the Task move: `lf task move`
  is the API that carries attribution; `lf resume --task X` on S follows the
  moved conversation.
- The log fallback triggers at receive, when M's reader finds no turns in the
  placed transcript (version skew). A provider that accepts the file but
  refuses the conversation at launch (the Codex cross-account case) is not
  detected; `lf -a <provider> resume` (`Resume::Import`) is the manual path.
- On the default branch a Session moves only when clean and is never committed
  to or pushed; a repository primary on `main` with edits refuses to move.

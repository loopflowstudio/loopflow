# Design assumptions and unresolved choices

- Human-confirmed on 2026-08-21: Discord is a symptom, not the priority. The
  primary gap is that the system itself must work; reporting UI depends on that
  operational truth. Discord remains useful as a perspective on where to look
  for non-convergence.
- Human-confirmed on 2026-08-21: this Task must answer the current-period
  question and build the tools needed to do so. It must not refine a future
  evidence command, DTO, or UI beyond what this specific investigation needs.
- The Task's two earlier Steers establish the seven-surface scope and authored
  Discord Red verdict. Future command, DTO, Podium, and publication-policy
  choices remain deliberately unmade.
- One exact 2026-08-20 User Ask now proves create → resolve → requester
  continuation, but finding that proof required a raw `ask_exchanges` read.
  Earlier abbreviated failure leads remain unresolved, and one success does not
  establish current-build or seven-day reliability.
- LOO-240 merged after the original evidence batch. The installed v0.12.12
  release is one commit behind it, so the current status failure and the
  delivered containment repair must remain visible at the same time.
- The minimal implementation is a task-scoped, read-only collector under
  `scripts/`, not a shared CLI projection. Its interface and output have no
  stability promise beyond reproducing this evidence period.
- Discord suppression remains a downstream hypothesis. A fresh User
  conversation would be needed to validate its exact boundary after execution
  reliably converges.

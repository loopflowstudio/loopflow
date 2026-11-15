# Reversible implementation choices

- LOO-905: reuse existing saved roadmap identity and live Task-list content;
  introduce no additional persistent title cache. Use Loading Task… when no
  readable identity exists during an initial read, and a generic Task unavailable
  heading only after a real failure. No product choice blocks implementation.
- Selection-only retained evidence has no independent current reading: show
  Updating… rather than borrowing the roadmap's failure. Task subscriptions
  retain their own reading state, including failure and identical-row recovery.

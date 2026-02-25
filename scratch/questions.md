# Open questions

- Confirm whether any wave/session execution path writes to a detached workspace that could drop `wave/<name>/MEMORY.md` edits before they are persisted. If yes, add an explicit post-step/session flush path.

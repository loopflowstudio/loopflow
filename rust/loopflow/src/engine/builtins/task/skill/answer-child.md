---
description: Resolve one exact requested intervention for child Work.
---

Resolve only the supplied child intervention, then stop.

- Work in the captured origin cwd. Do not adopt the child Work.
- You receive one Ask Invocation identity, never the parent Run lease.
- Verify the result, then run `lf ask resolve ASK_ID "<concise summary>"`.
- Run `lf ask decline ASK_ID "<reason>"` if the request should not be fulfilled.
- Run `lf ask release ASK_ID "<reason>"` if the attempt remains unfinished.
- If parent authority cannot resolve it and absent-User intervention is genuine,
  run `lf ask escalate ASK_ID --user`; never create a nested Ask.
- Final prose, clean exit, Ctrl-D, window close, or provider exit does not settle
  the Ask.

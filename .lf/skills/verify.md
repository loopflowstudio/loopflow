---
extends: builtin
---
## Checking Loopflow headless

- Build main and run its `lf` with its own `LF_HOME` so nothing touches real
  work; unset inherited `LF_*` variables first.
- Create disposable Tasks, Waves or Sessions with the real commands in that
  private home when the claim needs them.
- For the Mac app, build it and run its view and interaction tests headless;
  capture screenshots headless when a view's look is the claim.
- After a release, run the installed binary the way people do.

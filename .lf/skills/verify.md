---
extends: builtin
---
## Checking Loopflow headless

- Build main and run its `lf` with its own `LF_HOME` so nothing touches real
  work; unset inherited `LF_*` variables first.
- Seed a fake world when the claim needs Tasks, Waves or Sessions, and drive the
  real commands against it.
- For the Mac app, build it and run its view and interaction tests headless;
  capture screenshots headless when a view's look is the claim.
- After a release, run the installed binary the way people do.

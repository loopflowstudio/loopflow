# LOO-678 open questions and assumptions (2026-10-28)

- Assumed: a rebuild ends an idle Session even while a viewer is attached; a Session
  holding a turn or Graph step finishes it first. Simpler and bounded; revisit if ending
  attached Sessions on every `cargo build` during interactive QA breaks Jack's flow.
- Assumed: Sessions ended this way are not auto-resumed by `lf resume --all` (Desktop
  launch). Auto-resume under the new build would make Dev.app rebuilds seamless; it is a
  product choice left for Jack.
- Unresolved: which process issued the ~39/s launches, and the exact kernel trigger.
  Logs rotated; three small probe classes did not reproduce. The fix removes the
  precondition (owners outliving their image) and logs the next occurrence.

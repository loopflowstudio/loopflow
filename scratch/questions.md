# LOO-978 choices made without anyone here

- **Mechanism.** Dependency-tracked derived values in `observe`, chosen over a
  content-keyed memo of `resolve_layers` (robust to unobserved inputs, but
  parses per read, needs its own eviction, no help for the catalog) and over
  filtering each file where it is read (changes validation semantics).
- **`--setting` overrides bypass the cache** instead of joining the key. Reads
  "never cached into the shared result" literally.
- **The installed-harness PATH probe stays per call and joins the key.**
  Routing `which_on_path` through `observe` would make it a dependency and drop
  the per-call stats; not done because nothing in the sample points at it.
- **The live `sample` at 40-agent load is left to a person.** It needs a branch
  build as the Machine server while Sessions are live. The plan's headless
  stand-in is a timing of forced readings.
- **No baseline timing was run in kickoff**; the plan rests on the 2026-11-18
  sample in the Task brief and on reading the code.

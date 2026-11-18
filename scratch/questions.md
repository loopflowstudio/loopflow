# LOO-978 choices made without anyone here

## Kickoff

- **Mechanism.** Dependency-tracked derived values in `observe`, chosen over a
  content-keyed memo of `resolve_layers` (parses per read, needs its own
  eviction, no help for the catalog) and over filtering each file where it is
  read (changes validation semantics).
- **`--setting` overrides bypass the cache** instead of joining the key. Reads
  "never cached into the shared result" literally.
- **The live `sample` at 40-agent load is left to a person.** It needs a branch
  build as the Machine server while Sessions are live. The plan's headless
  stand-in is a timing of forced readings.
- **No baseline timing was run in kickoff**; the plan rests on the 2026-11-18
  sample in the Task brief and on reading the code.

## Concept review (2026-11-18), choices made for review

- **The key is a closed enum `Derived`, like `Fact`**, not a generic `K`. One
  list of what is cached and what identifies it. Passed over: generic key.
- **Dependencies nest by reference** instead of being copied into each reading.
  A catalog hit stays one insert. Passed over: flattening, which is simpler to
  write and costs every reading the catalog's several hundred facts.
- **`Outside` is both key and argument** for settings: environment, invocation
  overrides, the sole installed harness, `LF_HOME`, the remote profile.
  `resolve_layers` stops reading them itself. Passed over: listing them in the
  key while the computation reads them again.
- **`LoadError` derives `Clone`**; the slot keeps the error, not its message.
- **Three derived values, not five.** The Task agent is a field of the settings
  value. Placement waits for a sample that shows it.
- **Not run:** no timing and no build in this step. The cost split inside
  `resolve_layers` (how much is `validate_value`) is read from the code and the
  sample; slice 1 measures it.
- **Not fixed here, on the page:** validation per resolution instead of per
  file; `validate_value` deserializing a whole `Config` per leaf; served
  warnings going to the server's stderr.

## Implement (2026-11-18)

- **All three slices in one step.** Each compiled and passed its tests before
  the next; they are three commits.
- **`Derived::SkillAgents` instead of `WaveSetup`.** Graph and agent need no
  slot of their own once `Settings` is shared. Passed over: a slot holding a
  three-part tuple.
- **Environment overrides are part of the key; only `--setting` bypasses the
  shared value.** The environment is one value for a whole process, so it
  cannot be "one command's" override the way a thread's `--setting` is. The
  server README says both "belong to the command that carries them"; the
  result a reader sees is the same either way.
- **The settings key holds the checkout path as given**, not made absolute.
  Callers in a server pass absolute paths and the server never changes cwd.
- **A slot stays while a reading still holds its dependencies**, or for 300 s
  after its last use: the fact rule, applied to slots. Without the first half
  a quiet subscription would recompute on its next reading for nothing.
- **Open:** a personal settings file that fails to load now reports its own
  error variant (`io error: …`) where `PersonalSettings` wrapped it as
  `invalid graph: …`. No test pinned the old text.

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
  server README said both "belong to the command that carries them"; realign
  rewrote that sentence to match.
- **The settings key holds the checkout path as given**, not made absolute.
  Callers in a server pass absolute paths and the server never changes cwd.
- **A slot stays while a reading still holds its dependencies**, or for 300 s
  after its last use: the fact rule, applied to slots. Without the first half
  a quiet subscription would recompute on its next reading for nothing.
- **Open:** a personal settings file that fails to load now reports its own
  error variant (`io error: …`) where `PersonalSettings` wrapped it as
  `invalid graph: …`. No test pinned the old text.

## QA (2026-11-18)

- **A bare Task-detail read is one reading for the length of the call**
  (`observe::reading`). The plan said bare commands compute inline exactly as
  before; with the per-reading memo deleted that made `lf wave status` about
  20% slower over 30 Tasks. Within that call each file is read once. Passed
  over: putting the memo back.
- **`Derived::GlobalSettings` is a fourth variant.** The plan held placement
  back until a sample showed it; the sample showed the global file's
  resolution under Machine names instead (213 samples), not placement.
- **The global settings slot keeps the error's text**, not `anyhow::Error`
  (not `Clone`). Every caller already turns it into `StoreError::InvalidData`
  by its text.
- **Left to a person:** the `sample` on the real Machine under 40 agents, and
  Desktop itself. Both QA servers ran in private homes with 30 Tasks.

## Concept review of the built change (2026-11-18)

- **A checkout's `.git` becomes an observed dependency of its settings**, with
  a test. Passed over: leaving it (needs a repository without
  `.lf/config.yaml`, reaches a display only), and keying on the resolved
  private path. The stale case was read from the code, not reproduced.
- **`GlobalSettings` keeps one home and takes the profile from its key.**
- **Not built, on the page:** installed harnesses as an observed fact; a typed
  key per derived value; `load_wave_config`'s `Option`.

## Implement, second pass (2026-11-18)

- **`GlobalSettings` keeps the store's home, written into `outside.home`.**
  `store.home_dir()` is the database's directory and can differ from
  `LF_HOME`, so dropping the field and reading the ambient home was not safe.
- **`placement::read_at` now compares against the home it was given**, not
  `lf_home_dir()`, when deciding whether a forwarded profile replaces the
  global document. Same answer wherever the two homes agree.
- **The `.git` observation sits on the local path only.** `remote::layers`
  also calls `private_path`; a served request never has a forwarded profile.

## QA, second run (2026-11-18)

- **A Task's personal settings are found from its repository, not its
  worktree**, as on the installed `lf`. Passed over: falling back to the
  repository for every layer when the worktree is gone, which would add the
  main checkout's checked-in settings where the base shows none.
- **`Derived::Settings` gained `repository`** instead of a fifth variant: it
  is the same computation with one more input.
- **Not reached:** a store home that differs from `LF_HOME`, and a forwarded
  remote profile. Neither has a route from a private Machine's socket.

## Human review (2026-11-18), nobody present

- **Went on to gate.** Every behavior the Task names was seen working against
  the installed `lf`; nothing promised is missing. No person reacted; the
  six questions on `scratch/human-review.html` are unanswered.
- **Not seen by this step:** CI's suites. The run on `433c7879ab` is green with
  `rust-test`, `rust-lint`, `rust-build`, `python-test` and `macos` skipped.

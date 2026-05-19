# Open questions — embedded terminal build driver

Headless `review-design` reshaped the kickoff (see the design doc's banner).
The mechanics verified clean; the data model was over-elaborated. Executive
calls made below — each is the lower-drift option, but a human should sanity
these three before implementation locks them in.

## 1. `source == "palette"` — is the discriminator name right?

**Decision made:** lifecycle mode (auto-exit vs stay-alive) is encoded as a new
`TerminalSession.source` value rather than the kickoff's new `interactive: bool`.
`source` already discriminates `"wave_step"` / `"user_shell"`; it's a `String`
column, so a new value needs no Postgres migration.

**Soft spot:** the *name*. `"palette"` describes provenance (where it was
launched) and that's consistent with the existing values. But the *behavior* it
selects is "stay alive after the flow." If a future non-palette caller also
wants stay-alive, `"palette"` becomes a lie. Alternatives: `"interactive"`
(behavior-named, but collides with the existing `agent == "interactive"` value
and conflates two axes), or a second narrow field after all if provenance and
lifecycle genuinely diverge later. **Assumption:** one provenance == one
lifecycle for now; revisit only if a second stay-alive caller appears. Picked
provenance-named `"palette"` to match siblings.

## 2. `PaneConfig.terminalSessionName` — rename or just repurpose?

**Decision made:** store the lfd session **id** in the existing
`terminalSessionName: String?` field; delete the synthesized
`lf-{waveId}-{paneId}` fallback (`MultiplexerView.swift:240`). No new field —
"keep one implementation."

**Soft spot:** the field is named `...Name` but will hold an id. Honest fix is
renaming to `terminalSessionId`, but that's a `Codable` key change → persisted
`UserDefaults` layouts written by older builds won't decode the key. CLAUDE.md
says don't carry back-compat for internal formats, which argues for the clean
rename + accepting that pre-change layouts reset to blank panes once (cosmetic,
self-heals on next launch). **Assumption:** clean rename to
`terminalSessionId`, accept the one-time layout reset. Flagging because it's a
visible-to-the-user (Jack) state loss the morning the change lands.

## 3. Create-request shape: `agent` vs `model` field name

**Decision made:** the create request carries `agent: String`, passed to
`lf -m <agent>` and stored as `TerminalSession.agent` (header reads it). No new
`provider` field — it would duplicate `agent` across three layers.

**Soft spot:** the CLI flag is `--model` / `-m` and takes `harness[:model]`.
Naming the request field `agent` matches the stored column and the resolution
chain in `engine/launch.rs:138-147` (which calls it `agent`), but mismatches the
flag the value feeds. **Assumption:** field name `agent` (consistency with the
stored/displayed model wins over consistency with the flag spelling). Low risk;
noting for the DTO fixture author so the fixture key isn't bikeshed later.

---

None of these block implementation. They're the seams where a human's intent
would override the headless default. Absent correction, implement as assumed.

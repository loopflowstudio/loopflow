# LOO-728 acceptance — November 4, 2026

Jack Heart accepted Desktop-only automatic presentation; the canonical scope and
remaining proof are in [write-docs-first.md](write-docs-first.md). The earlier
configured Rex agent-route requirement is superseded for the workflow. Explicit
terminal opening remains supported. This evidence does not claim configured
Desktop acceptance or authorize parent landing, installation or owner restart.

## Historical explicit-opening evidence

Jack Heart requested a development CLI demo and confirmed “ok it opened”. The
build was `0.15.1+f20e05d1e.dirty`, including the repair that invokes the shared
editor directly instead of the older installed CLI's unsupported `task tool` hop.
`open --host rex --task LOO-728 --file docs/orchestration.md --json` and a repeat
adding `--file docs/surfaces.md` ran with `env -u LF_AGENT_CALLER`. They prove the
**direct CLI path**, not the configured agent route. No installed binary was
replaced or owner restarted.

Native inventory, capture and screenshots showed the original files beside the
same conversation. Repeat opening reused the orchestration editor and retained
conversation focus; both docs remained unchanged. [demo.html](demo.html) retains
the walkthrough and screenshots. The later inventory contained only the original
conversation block; this demo issued no close command. The disappearance has no
established cause, and recovery bytes remain preserved.

- Window: `window:01a1a819-d002-7cb2-8ee8-713b924e42ba`.
- Conversation: `block:01a1a819-d001-794e-90aa-967b912ab9b5`.
- Orchestration editor: `block:01a1a852-f45f-7461-9aa3-bad70c06fa9b`.
- Complete direct-demo evidence: `ef0ca500977a123076cf08f0e114b7fd59d36881:scratch/docs-first-dev-demo.md`.

The configured agent attempt failed because the older owner answered
`{"answers":1,"seq":0,"kind":"controlled","held":true}` without placement.
The candidate refused stale startup placement; no editor or configured
readback/draft/continuity proof resulted. An isolated real-Rex candidate-owner
probe did preserve original-file editing, focus and an unsaved correction on
repeat, save/read the correction and recover missing-file failure. That is bounded
explicit-opening evidence, not configured installation or acceptance.

Full Git history repaired candidate migration ordering. The catalog at
`c136addfa` rebuilt, but promotion remained `ahead_pending` with only
`rex_workspaces` unmerged; no shared store was modified. The later parent
`df3b5e810d3841a19bc5911c332fb42f23193d46` is now integrated and rebuilt with
its revised schema and installer. The old preflight describes the previous schema;
no configured demo was repeated.

Full observations and unpublished diagnosis:
`5d2f6f21eca663c16be3b8e3459d9304fc13c8ed:scratch/docs-first-demo.md`.
Earlier archive: Git blob `3c105e3f2c64cfdd313e0fd6c0303f1971a4e13d`.

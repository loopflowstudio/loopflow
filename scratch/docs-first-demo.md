# LOO-728 evidence boundaries

Jack Heart accepted Desktop-only automation and independent-main delivery on
November 4, 2026. The current implementation plan is `write-docs-first.md`.

Earlier configured candidate commands returned the original documentation paths
with `host:null` and `opening:false` while Desktop refused connections. Missing
paths failed; retry preserved document hashes and created no host or Session.
Those observations belong to stacked `81cf57afe9`, not the extracted tree.
Earlier explicit Rex opening succeeded directly, but the configured agent route
failed on the old transport response. This is not proof of attached Desktop.

The previous gate passed 252 selected Rust checks after host-isolation repair,
website checks and 105 focused Files/pane/DTO tests. Its full Swift run exited
without a reported assertion or timeout; 21 isolated Theme tests subsequently
passed without explaining that exit. These are prior-tree evidence only.

Full commands, hashes, screenshots, native IDs, failure details and decisions:
`ea0e0781b22f092b6a0169802997ff7a3628c65c:scratch/docs-first-demo.md` and
`scratch/write-docs-first.md` at that same commit. Source and parent work remain
preserved; there is no authorization here to install a build or restart an owner.

The earlier demo returned to implement because detachment had not occurred;
detachment and publication have since completed (see `write-docs-first.md`).
The remaining walkthrough concerns the independent candidate. Attached rendering, active
draft retention on repeat, UI save/readback and conversation continuity remain
unobserved; headless equivalents are automated evidence, never mounted UI proof.

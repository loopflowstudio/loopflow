# LOO-717 — Session viewer and shell lifetime demo

November 5, 2026. Jack Heart invoked demo for the published change at
`500425244c`. Design: [shell lifetimes](loo-717-shell-lifetimes.md).
Review page: `scratch/demo.html` (written after the demonstration record).

## Scope and decision

Review the actual candidate's Desktop shell reconnect/Close and LF Task Session
viewer retention. No acceptance, shipping or installation has been given in this
review. Published automated passes remain separate from rendered demonstration.

The old candidate at `500425244c` retains unknown remote viewer loss indefinitely.
Jack subsequently accepted bounded disconnect cleanup and explicit Quit release;
that repair now passes focused Rust checks, real loopback SSH cycles, Clippy and
the Swift build in the current checkout. The private copied app remains the old candidate. Rebuild the private
candidate before demonstrating the revised contract. The old candidate does not include LOO-672's later automatic
conversation opening or LOO-498's layout launcher; native Rex can launch the demo
viewer directly.

## Private roots and provenance

- Durable root: `/Users/jack/.lf/demos/loo717-500425244c/durable`
- Runtime root: `/Users/jack/.lf/demos/loo717-500425244c/runtime`
- Private Machine: durable root `/machine` (`LF_HOME`).
- Candidate: runtime root `/Loopflow LOO-717 Demo.app` (whole copied bundle).
- Copied CLI: candidate `/Contents/MacOS/lf` (`LF_BIN` for all fixture operations).
- Fixture repository and local bare origin live under the durable root.
- Fixture uses local Git and disposable Task records, not a GitHub publication.
  Any seeded merged-delivery record is fixture setup, not a live PR merge proof.
- Rust CLI/FFI and Swift builds completed November 5. Jack then invoked
  concept-review; private fixture preparation and app/provider launch have not
  run. The review found consistency repairs in the working design. Exact
  Session/owner identities, screenshot paths,
  observations and commands will be recorded below before review handoff.

## Input and teardown

No user input or demo provider is launched yet. Preserve both roots while owners
live or review input is unresolved. Never delete durable history. End only verified
demo owners after review input is settled; an edited draft needs Jack's explicit
release in this conversation. Closing a viewer is not that release. The page will
ask for that release separately from behavioral acceptance.

## Runbook

1. Open the disposable Task's Desktop shell. Set a variable and note its PID;
   hide/switch/reconnect. Expect the same PID and variable. Close should end that
   shell/jobs; Undo should not restart them.
2. View the disposable Task Session in Rex, with an unrelated shell in its
   checkout. Complete only this fixture Task. Expect the LF viewer to retain the
   checkout even when unfocused. A second passive viewer also retains it.
3. Close/detach the last LF viewer. Expect ordinary fifteen-minute eligibility;
   the unrelated shell must neither retain the checkout nor receive LF Close.
   Any accelerated deadline must be labeled explicitly, not reported as a
   fifteen-minute timing proof.
4. Observe active-work protection and bounded remote disconnect cleanup where configured proof is
   available; carry absent physical remote-Machine evidence as unproved.

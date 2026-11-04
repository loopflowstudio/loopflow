# LOO-728 development CLI demo — 2026-11-04

Historical explicit-opening evidence. Jack Heart subsequently accepted Desktop-only
automatic presentation; `write-docs-first.md` owns current scope. The pending
choice recorded below is resolved and adds no current acceptance blocker.

Jack Heart requested a demo using development lf. He subsequently observed:
“ok it opened”. This confirms visible opening, not acceptance of every remaining
Task claim. Earlier ship authorization remains in effect; the live Graph owns
implementation and delivery.

Built with `RUST_LOG=warn scripts/dev-lf --version` from this checkout. Executable
reported `lf 0.15.1+f20e05d1e.dirty`; the dirty tree includes the live repair.
This is development behavior, not the published f20e05d1e implementation or an
installed release. The repair launches the shared editor directly, eliminating
the older installed lf's unsupported `task tool` hop.

## Observed in the configured Rex app

Ran the branch executable with `open --host rex --task LOO-728 --file
 docs/orchestration.md --json`, then repeated that file together with
`--file docs/surfaces.md`. These commands used `env -u LF_AGENT_CALLER` to
exercise the **direct CLI path**; they do not prove the agent-caller route.
No installed binary was replaced, and no current Session owner was restarted.

Both commands returned `opening:true`. Rex native inventory and terminal capture
showed a real editor displaying the original orchestration file. The second
request reused that editor and added the surfaces editor. Native app inspection
and a screenshot then showed both original documents beside this conversation.
The original conversation block retained focus across file presentation.
`git diff --exit-code -- docs/orchestration.md docs/surfaces.md` passed.

- Window: `window:01a1a819-d002-7cb2-8ee8-713b924e42ba` (LOO-728 Docs first).
- Existing conversation: `block:01a1a819-d001-794e-90aa-967b912ab9b5`.
- Orchestration editor: `block:01a1a852-f45f-7461-9aa3-bad70c06fa9b`.
- Before and after native screenshots: `docs-first-demo-assets/before.png` and
  `docs-first-demo-assets/after.png`.
- Interactive runbook: `demo.html`.

The editors were left open at the end of presentation. A later native inventory
showed only the original conversation block; this conversation issued no editor
close command. Do not attribute that disappearance or infer editor acceptance.
A remaining editor swap file is not ours to delete.

## Feedback and remaining proof

Jack Heart asked about better rendered Markdown and HTML viewers and clarified
that they must work cross-platform and cross-machine. His stray “vim” message
was withdrawn: “ignore that vim stuff”. He then proposed considering automatic
file opening only in Loopflow Desktop. That scope decision is pending here.
No viewer replacement, dependency installation or new product scope is approved
by that exploration alone. The installed Rex creator inventory exposes terminal
and onboarding blocks; it exposes no browser/Markdown block.

The live repair worker separately reports an isolated candidate-owner test with
real editors, draft preservation and saved-file readback. This primary demo did
not repeat those edits on Jack's actual documents. The existing conversation's
older owner cannot establish the repaired viewer-query protocol. Preserve that
limit and the failure evidence in [docs-first-demo.md](docs-first-demo.md).
The accepted design remains [write-docs-first.md](write-docs-first.md).

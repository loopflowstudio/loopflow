# Configured host and shared contract proof — 2026-09-24

The current UI boundary is a locked macOS desktop. The exact runner is
Accessibility trusted; system AX focus belongs to PID 395, `loginwindow`.
A current signed disposable build is ready. No production change is justified
by this host observation.

## Configured observation

Built the current SwiftPM product with:

```sh
swift build --package-path swift -Xswiftc -gnone --jobs 4 --product LoopflowMac
```

The build passes. Copied its executable into
`/tmp/loo291-iteration16/Loopflow Proof.app`, preserving the existing packaged
resources and matching local CLI selection. Updated the disposable bundle's
identifier and display name and signed it ad hoc; strict signature verification
passes. No installed binary or human demo was replaced. The source, CLI and
signed executable hashes are in the [receipt](configured-ui-evidence/iteration16/receipt.json).

Three bounded, read-only launches established distinct facts:

1. PID 87766: AX trust true. AXWindows contains one element, but that element's
   role is AXApplication and it equals the application root. It supplies menus,
   not workspace controls. Counting it as an accessible window was incorrect.
2. PID 8698: moving observation one second after the launch callback produces
   the same result. This does not support the early-callback hypothesis.
3. PID 44369: direct console inspection reports `CGSSessionScreenIsLocked = 1`;
   system AX focused application is PID 395, independently identified as
   loginwindow. WindowServer lists an owned onscreen window while AX still
   returns the application as its window. The attempted capture of the first
   owned layer-zero window fails; no screenshot or appearance verdict exists.

Each owned app accepted termination and all three PIDs were absent afterward.
No keyboard input, AX action, provider launch, Session open, transfer, resolution
or PM edit occurred. These observations establish this host's current boundary;
they do not retroactively explain every earlier inactive/empty-tree observation.
[Initial](configured-ui-evidence/iteration16/observe.log),
[deferred](configured-ui-evidence/iteration16/deferred-observe.log),
[direct host](configured-ui-evidence/iteration16/window-observe.log).

## Live contract comparison

Captured fresh `roadmap --all --json` and `session list --json` commands against
the same explicitly selected development Home as the review app. These read
cached planning; no Linear sync or fresh provider-authorship claim is made.
The comparison selects Loopflow's three repository Waves from the all-Wave
response and uses the repository-scoped Session response.

The [standalone comparator](configured-ui-evidence/iteration16/compare.swift)
compiles alongside production `WorkStatus.swift`, `WaveWorkMap.swift` and
`SessionRecord.swift`. It compares raw Rust JSON directly with decoded values:

- Five Projects: identity/name/summary/definition, runtime Work ID and all seven
  KR texts/holds values.
- 145 Tasks: identity/identifier/name/directive/completed, runtime Work and
  Project IDs, condition/reason/time/age, recommended action/reason, checkout
  path and local existence.
- Four Sessions: every encoded wire field equals the raw payload, normalizing
  only absent versus explicit-null optional fields. The Task-attributed Session
  joins its exact runtime Work ID to planning identity and expected readable
  Wave/Project/Task path. The other three records remain unmatched and preserved.

**All 154 records pass.** The observed Task conditions are waiting and blocked;
all four Sessions are interactive. This exceeds twenty individual live record
comparisons, but supplies no configured UI trials, all-kind action coverage,
external-work outcome, paint budget or interactive-readiness measurement.
Unseen states remain covered only by the earlier shared fixtures and focused
behavioral proofs. [Comparison ledger](configured-ui-evidence/iteration16/comparison.json),
[pass](configured-ui-evidence/iteration16/compare.log),
[read provenance](configured-ui-evidence/iteration16/read-receipt.json).

## Resume the configured proof

Unlock this Mac, then run from the assigned checkout:

```sh
xcrun swift scratch/configured-ui-evidence/iteration16/ready-probe.swift
```

This rerunnable read-only probe aligns the Home, database and CLI, reads the
current Session population, launches only its own disposable app, requires an
actual AXWindow and the matching scoped count, checks population stability,
and terminates only that app. It never opens, moves or resolves a Session.
The lock check now exits before launch on this host (exit 2, trusted runner);
that is a correct unavailable outcome, not a UI pass. The unlocked path is not
yet exercised. No permission change or bypass is requested by current evidence.

After that check succeeds, use newly owned disposable conversations for the
remaining configured shared controls and nested keyboard/draft/shell proof.
Do not replay the retired mutation probes or use an arbitrary existing Session.
Human-selected external Work and its exact directive edit remain unprovided;
the pending clarification asks for both. The ten external trials and published
paint/interaction budgets with twenty registry trials remain outstanding.

## Review and disposition

The useful review finding was in the proof: an AXWindows array length cannot
prove an accessible window. The revised probe checks role and current host state
before treating a count observation as a UI result. No product workaround, new
state owner or lifecycle policy was added. The live comparisons assert exact
source values without deriving condition or Session legality in the presenter.

No production edits or broad test gate occurred. The prior focused tests retain
their recorded source scope; they are not new iteration-16 results. The current
build, signature verification, live comparison and `git diff --check` pass.
No publication, landing or Task completion is established.

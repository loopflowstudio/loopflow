# LOO-718 native iOS implementation

## Overnight direction — November 3, 2026

Jack Heart asked for sustained autonomous improvement toward an excellent iOS app, expanding toward Desktop capabilities where useful while prioritizing polish, performance and UX over feature count. The offline slice is a starting point, not the completion boundary. Continue in this checkout; neither the earlier demo stop nor absent feedback ends implementation. No request to merge was made.

Next work: improve Task-list density/search and honest unavailable states; make terminal keyboard recovery and spacing comfortable; then join the actual Machine contract and authenticated host-Tailscale transport. LOO-593's current published branch now admits view_plan and repository_path, superseding the missing-view-plan limitation recorded below. Revalidate actual source and fixtures at integration. TestFlight remains LOO-721; physical-device and live-host evidence are still required. Favor a complete Task → usable Session over additional panes. Record observed behavior separately from product acceptance.

### November 3 overnight checkpoint

The compact searchable Task list, honest missing-Session screen, terminal insets
and fresh-grant keyboard restoration are implemented. The renderer now preserves
reported owner columns/rows while observing and fits the local viewport only with
geometry ownership. A wrapping scroll view pans that fixed canvas; local size
changes are remembered without sending observer resizes. Duplicate geometry
reports and repeated theme installation are eliminated. The shared owner still
owns all authority; this introduces no serialized domain types or retention policy.

The mounted UIKit tests prove 120 × 40 survives phone layout changes, observer
typing is ignored, ownership fits the phone, terminal input modes survive and
unchanged layouts send no extra resize messages. Manual Simulator inspection
verified Task-list density, missing-Session explanation and keyboard-open fitting.
Touch panning was not established by the automation gesture; keep it as real-device
proof rather than equating programmatic content-offset tests with gesture quality.

Next: consume the actual owner frames (including resize-before-render ordering,
nonce-qualified events and bounded streaming), add authenticated host-Tailscale
admission to the existing service, and join actual Task reads/Session attachment.
The backend lives on LOO-593's branch at 07a6a184d4, not in this checkout yet;
integrate that dependency instead of writing another Machine implementation.

Check: the README's focused native command passed 16 tests with zero failures
at 23:40 on November 3; result `swift/.build/ios/Logs/Test/Test-LoopflowiOS-2026.11.03_23-40-33--0800.xcresult`.
This includes programmatic viewport panning without grid changes. Search was
also exercised in Simulator: “review” leaves only Review a change, which opens
its Session; dismissing search restores all three Tasks. Updated captures are
`demo-assets/polished-tasks.png` and `demo-assets/polished-keyboard.png`.

Jack Heart accepted existing open Tasks → the selected existing Session terminal,
Desktop styling, and a reduced Paste / New line / keyboard row. Jack invoked
launch-plan on November 3, 2026: implement this native lane in the existing
checkout alongside LOO-593, stopping for the feature Graph's interactive demo.
No Auto-Ship approval. No Wave navigation, shells, inspectors or new-Task UI.
Reconciled November 3, 2026 against recovery commit `81f3a30854` and its test result.
The native fixture exists; device acceptance and live integration remain open.

## Selected implementation

The iOS Xcode target lives in `swift/ios.yml`, including the shared project without
editing Desktop's package/project. SwiftTerm 1.20.0 is pinned by revision; the
UIKit terminal supplies committed input, IME, selection and accessibility.
`LoopflowiOS/README.md` records the renderer evidence and exact focused command.
Ghostty's existing artifact is Mac-only. Lakr's wrapper was inspected at
`db22505bd74d968c7fc12393219420ecbcead393`: host I/O and UIKit input exist, but
Swift 6.2/module collision/accessibility integration cost favors this SwiftTerm
slice. No Ghostty simulator/device performance comparison was performed.

`MobileClient` is an in-process UI dependency, not a wire protocol. Task fixtures
decode through LOO-593's exact `MachineProtocol.swift` and existing WorkViewPlan.
The consumed codec and six fixtures still match LOO-593's November 3 working files
at their canonical paths; LOO-593 owns them. Its contract supports roadmap
subscriptions, desktop_sessions reads and Session→owner lookup, but not view_plan.
The phone's task-reading.json is client-authored WorkViewPlan data inside the
shared envelope, not a recorded server response. Shared owner/command fixtures
decode unchanged; the app does not yet issue owner lookup or attach requests.
The bridge switches from length-prefixed JSON to the existing binary owner
protocol after bridged. Live primary-Session selection and that decoder remain
to be joined; fixture success establishes neither admission nor connectivity.

Opening observes. The fixture-only **Use keyboard** action grants control in a
single-client peer; it proves no contention or remote takeover UX. Input and
geometry are separate permissions. Reconnect returns to the same Session as an observer and never
replays input. Backgrounding releases the attachment. The deterministic peer
owns its editor draft, processes native bytes and emits VT output; it executes
no commands. The app visibly says Offline fixture on both screens and is
unavailable without the explicit fixture launch argument.

Shared fonts/themes supply Desktop styling. Permission questions are answered
inside the terminal. Conditional native approval and Stop buttons wait for
trustworthy provider state plus supported action bindings; arbitrary screen text
cannot create them. Multiline paste requires bracketed paste support.

## Ownership and retired approaches

No production iOS predecessor exists. One renderer and the shared Machine codec
serve this target. Task rows use direct navigation links; the redundant destination
and presentation flag were removed. The Session screen owns attachment lifetime.
HTML studies remain historical interaction evidence, never an alternate product
path. Detailed research is preserved at
`9e4587bb53fc4102e8682a3bba61e9776a6de5c2:scratch/`; current notes summarize it
without removing original assets or HTML. LOO-593 owns Work FFI removal; this lane
does not repair or duplicate that implementation.

## Recovery correction — accepted November 3, 2026

The headless concept review applied Jack Heart's same-Session/no-replay contract;
`scratch/concept-review.html` preserves the original defect and corrected status.
Attachment exceptions mapped to `unavailable`, hiding Reconnect after an initial
or repeated failure.
The correction uses `disconnected` and the existing in-screen action. It creates
no client draft, recovery record, domain DTO, automatic retry or owner resolver.

`MobileWorkspace.open` now maps attachment exceptions to `disconnected`.
Explicit disconnect invalidates the attachment generation before closing it;
stale events and completion share that lifetime check. No recovery code remains
to implement for this finding.

Recovery stays on the selected Session screen and restores viewing, with input
and geometry disabled until a fresh grant. The README owns usage instructions.
Retained output while connecting is previous output, not current-state evidence.
Passing tests exercise the production Reconnect button through two failures and
success. Accepted text and Return followed by send errors preserve the peer draft
and exactly one submission. Explicit drops and stream completion both disconnect.
Wrong-Session output never renders and its attachment closes. Missing/invalid
identity and owner termination retain unavailable/ended explanations and no retry,
including after backgrounding.

LOO-593 still owns error/owner truth. Its future adapter must preserve authoritative
permanent failures rather than treating them as transport exceptions. This fixture
correction chooses no carrier/pairing, remote takeover gesture or hosting tier.
Physical input judgment remains with demo; live connectivity remains integration.

## Concept review disposition — November 3, 2026

Nothing further to change in the accepted native fixture design; no open product
choice within this slice. Jack Heart's two-screen interaction now retains its
recovery action. Task and Session identity survive attachment replacement;
the peer retains submitted input and its draft, while control and geometry grants
expire. Missing, ended and mismatched Sessions remain counterexamples to treating
every failure as reconnectable. The existing README accurately describes this
flow; no usage or skill rewrite is needed. No implementation changes are proposed.

LOO-593's published local contract at `5b44a394cff91d0aa20c614c3b20c3f7e121affb`
matches the consumed Swift codec byte for byte. Its owner lookup and binary bridge
preserve the same ownership model; publication does not supply remote admission
or a live WorkViewPlan reading. Carrier/pairing, remote takeover and hosting remain
unresolved for follow-up C, as already scoped. Next proof remains the interactive
native demo and the listed integration work, not another fixture redesign.
The saved 21:06:45 xcresult confirms 14 passes and zero failures; this review
changes only design prose and its page, so the suite was not rerun.

## Remaining proof and integration

- Device/demo: real keyboard-open layout, CJK keyboard conversion, dictation,
  physical keyboard combinations, VoiceOver navigation, and output/energy use.
  Mounted simulator tests exercise UIKit input APIs and accessibility reading;
  they are not physical-device experience evidence.
- LOO-593 joining: supported primary-Session read and use of its exact owner lookup;
  shared terminal frame/event decoder and recorded snapshots; replacement
  subscription readings, stream gaps, bounded queues and cancellation. Current
  fixture uses the real Machine envelope, but terminal events remain in-process.
  The existing same-user Unix socket is not a remote endpoint; forwarding it
  through Tailscale would bypass the separate remote admission boundary.
- Live follow-up C: carrier/pairing/admission, canonical observer geometry,
  elected terminal replies, Desktop handoff, suspend/reconnect beyond retention
  and real host Tailscale. Never claim this fixture is live mobile completion.
- Full Work cutover and describe-first creation remain separate outcomes in
  mobile-task-plan.md; no additional Task or Graph was launched here.

Main sync includes terminal activity #1809/LOO-720 at ac8e088f5f. Session identity
#1807/LOO-582, Task selection #1791/LOO-673 and DTO revert #1808 remain coordination
references; their delivery state was not established here. LOO-717/709/710/712
retain shell, retention, snapshot and transport ownership. No policy changes in this lane.

Check: `xcodebuild test -project swift/LoopflowMobile.xcodeproj -scheme LoopflowiOS -destination 'platform=iOS Simulator,name=iPhone 17 Pro' -derivedDataPath swift/.build/ios -skipPackagePluginValidation -parallel-testing-enabled NO -only-testing:LoopflowiOSTests/MobileTerminalTests CODE_SIGNING_ALLOWED=NO` — 14 passed, November 3, 2026, 21:07; reused after reading the xcresult summary, with code unchanged. Broader checks remain with gate and device/live proof with demo/integration.

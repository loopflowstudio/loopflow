# LOO-718 native iOS implementation

Jack Heart accepted existing open Tasks → the selected existing Session terminal,
Desktop styling, and a reduced Paste / New line / keyboard row. Jack invoked
launch-plan on November 3, 2026: implement this native lane in the existing
checkout alongside LOO-593, stopping for the feature Graph's interactive demo.
No Auto-Ship approval. No Wave navigation, shells, inspectors or new-Task UI.

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
Canonical files and machine fixtures were consumed at their repository paths;
LOO-593 remains their owner. The backend's current local allowlist covers roadmap,
not view_plan; live primary-Session selection still needs joining. Its new
Session→owner lookup and command trace fixtures are consumed unchanged. Fixture success establishes neither admission nor connectivity.

Opening observes. The fixture-only **Use keyboard** action requests a non-forcing
grant; this does not settle remote takeover UX. Input and geometry are separate
permissions. Reconnect returns to the same Session as an observer and never
replays input. Backgrounding releases the attachment. The deterministic peer
owns its editor draft, processes native bytes and emits VT output; it executes
no commands. The app visibly says Offline fixture on both screens and is
unavailable without the explicit fixture launch argument.

Shared fonts/themes supply Desktop styling. Permission questions are answered
inside the terminal. Conditional native approval and Stop buttons wait for
trustworthy provider state plus supported action bindings; arbitrary screen text
cannot create them. Multiline paste requires bracketed paste support.

## Delete — do not maintain

No production iOS predecessor exists. Keep one renderer and one Machine codec.
Task rows use direct navigation links; do not maintain a separate destination
and presentation flag. The Session screen owns opening and leaving its attachment.
HTML studies remain historical interaction evidence, never an alternate product
path. Detailed research is preserved at
`9e4587bb53fc4102e8682a3bba61e9776a6de5c2:scratch/`; current notes summarize it
without removing original assets or HTML. LOO-593 owns Work FFI removal; this lane
does not repair or duplicate that implementation.

## Remaining proof and integration

- Device/demo: real keyboard-open layout, CJK keyboard conversion, dictation,
  physical keyboard combinations, VoiceOver navigation, and output/energy use.
  Mounted simulator tests exercise UIKit input APIs and accessibility reading;
  they are not physical-device experience evidence.
- LOO-593 joining: supported primary-Session read and use of its exact owner lookup;
  shared terminal frame/event decoder and recorded snapshots; replacement
  subscription readings, stream gaps, bounded queues and cancellation. Current
  fixture uses the real Machine envelope, but terminal events remain in-process.
- Live follow-up C: carrier/pairing/admission, canonical observer geometry,
  elected terminal replies, Desktop handoff, suspend/reconnect beyond retention
  and real host Tailscale. Never claim this fixture is live mobile completion.
- Full Work cutover and describe-first creation remain separate outcomes in
  mobile-task-plan.md; no additional Task or Graph was launched here.

Preserve Session identity work #1807/LOO-582, terminal activity #1809/LOO-720,
Task selection #1791/LOO-673 and DTO revert #1808. LOO-717/709/710/712 retain shell,
retention, snapshot and transport ownership. No policy changes in this lane.

Check: `xcodebuild test -project swift/LoopflowMobile.xcodeproj -scheme LoopflowiOS -destination 'platform=iOS Simulator,name=iPhone 17 Pro' -derivedDataPath swift/.build/ios -skipPackagePluginValidation -parallel-testing-enabled NO -only-testing:LoopflowiOSTests/MobileTerminalTests CODE_SIGNING_ALLOWED=NO` — passed, 11 tests; device/live proof stays with demo and integration.

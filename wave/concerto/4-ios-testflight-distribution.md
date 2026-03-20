---
asana_id: '1213718347030073'
linear_id: 33d27810-bdc7-499e-ba8a-1e0843df52fc
---
# 06: iOS TestFlight Distribution

**Finish line:** Concerto's iOS target builds in CI, uploads to TestFlight, and the installed app can sign in, discover `lfd`, and connect on a real device.

## Carried context

- The project already has multiplatform Swift and XcodeGen targets for iOS.
- iOS discovery is the supported connection path; the manual host/token flow was deleted.
- The project is configured for team `2V3M244HF2` and bundle ID `com.loopflow.concerto`.
- The macOS release workflow is the closest existing template.
- Headless validation is still noisy: `xcodebuild test -scheme Concerto -skip-testing:ConcertoUITests` currently tries to link `ConcertoUITests` anyway and fails before iOS distribution work can reuse that lane.

## What to build

1. Create the App Store Connect record and signing setup for `com.loopflow.concerto`.
2. Extend the release tooling to archive an iOS build and upload it to TestFlight.
3. Add a CI path for manual or scheduled iOS uploads.
4. Verify iOS entitlements, Info.plist settings, and URL-scheme behavior on device.

## Done when

- `xcodebuild archive` produces a signed iOS archive.
- A TestFlight build is uploaded and installable.
- Discovery works on device: sign in, find `lfd`, connect, and view waves.

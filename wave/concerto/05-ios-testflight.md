# 05: iOS TestFlight Distribution

**Status:** Backlog. No distribution pipeline exists. Source code is multiplatform-ready.

## Problem

Concerto has iOS source code (Platform/iOS/ views, multiplatform targets, iOS 18.0 deployment target) but no way to get it onto a phone. No TestFlight, no provisioning profiles, no iOS CI.

## What exists

- Multiplatform Swift package and XcodeGen project (macOS + iOS)
- iOS views: DiscoveryView, MobileRootView, MobileWaveListView, MobileWaveDetailView
- Team ID `2V3M244HF2` in project.yml with automatic signing
- Bundle ID `com.loopflow.concerto`
- macOS release pipeline (release.yml, release-concerto.py) as reference

## What's needed

1. **App Store Connect** — create app record for `com.loopflow.concerto`
2. **Provisioning** — iOS distribution certificate + provisioning profile (automatic signing may handle this)
3. **Build script** — extend `scripts/release-concerto.py` or new script for iOS archive + TestFlight upload
4. **CI workflow** — GitHub Actions job: build iOS, archive, upload to TestFlight (manual trigger initially)
5. **Entitlements** — review `Concerto.entitlements` for iOS-specific needs (currently macOS-only audio input)
6. **Info.plist** — add `LSMinimumSystemVersion` equivalent for iOS, verify URL scheme works on iOS

## Done when

- `xcodebuild archive` produces an iOS .xcarchive
- TestFlight build uploaded and installable on device
- Discovery flow works: sign in → find lfd → connect → see waves

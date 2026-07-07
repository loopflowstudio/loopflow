#if os(macOS)
import AppKit
import Foundation
import XCTest

// The Loopflow unit tests are hosted inside the Loopflow GUI app. When the test
// bundle finishes, the app's SwiftUI/AppKit run loop keeps spinning, so
// `xcodebuild test` never exits even though every test passed — the CI job then
// hangs until it is force-killed.
//
// This observer terminates the host as soon as the bundle finishes.
// `testBundleDidFinish` fires after BOTH the XCTest cases and the Swift Testing
// suites complete (verified against the run log: it lands after the final
// "Test run with N tests ... passed" line), so terminating here never truncates
// a test.
//
// It uses `NSApp.terminate`, not `exit()`: a raw `exit()` here makes xcodebuild
// think the runner died mid-session ("exited with code 0 before finishing")
// and it fails after retrying. `NSApp.terminate` drains the run loop first, so
// xcodebuild's IPC teardown completes and the run reports success (exit 0).
//
// Registered as the LoopflowTests bundle's NSPrincipalClass (see project.yml).
// XCTest instantiates the principal class once at bundle load, so registration
// is guaranteed — unlike a lazily-initialized global that nothing references.
@objc(LoopflowTestsPrincipal)
final class LoopflowTestsPrincipal: NSObject, XCTestObservation {
    override init() {
        super.init()
        XCTestObservationCenter.shared.addTestObserver(self)
    }

    func testBundleDidFinish(_ testBundle: Bundle) {
        DispatchQueue.main.async {
            NSApp?.terminate(nil)
        }
    }
}
#endif

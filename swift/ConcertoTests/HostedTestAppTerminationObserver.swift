import XCTest

#if os(macOS)
import AppKit
import Foundation

private final class HostedTestAppTerminationObserver: NSObject, XCTestObservation, @unchecked Sendable {
    func testBundleDidFinish(_ testBundle: Bundle) {
        Task { @MainActor in
            guard NSApp != nil else { return }
            FileHandle.standardError.write(Data("ConcertoTests: terminating hosted app after test bundle finished\n".utf8))
            NSApp.terminate(nil)
        }
    }
}

private let hostedTestAppTerminationObserver: HostedTestAppTerminationObserver = {
    let observer = HostedTestAppTerminationObserver()
    XCTestObservationCenter.shared.addTestObserver(observer)
    return observer
}()
#endif

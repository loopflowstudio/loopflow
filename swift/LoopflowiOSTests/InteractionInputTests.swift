import Loopflow
import UIKit
import XCTest
@testable import LoopflowiOS

@MainActor
final class InteractionInputTests: XCTestCase {
    private final class ClockEvent: UIEvent {
        override var timestamp: TimeInterval { 42 }
    }

    private final class Recipient: NSObject {
        var input: InteractionInput?
        @objc func pressed() { input = InteractionTrace.input }
    }

    func testNativeActionKeepsEventClockAndRestoresCaller() {
        XCTAssertTrue(UIApplication.shared is InputApplication)
        let recipient = Recipient()
        XCTAssertTrue(UIApplication.shared.sendAction(#selector(Recipient.pressed), to: recipient, from: nil, for: ClockEvent()))
        XCTAssertEqual(recipient.input, InteractionInput(uptime: 42, source: .uiKitEvent))
        XCTAssertNil(InteractionTrace.input)
    }

    func testNoCaptureSessionLeavesVisualTimingUnavailable() async throws {
        let window = try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
            .flatMap(\.windows).first { $0.isKeyWindow })
        XCTAssertFalse(InteractionFrame.isEnabled)
        let captured = await InteractionFrame.capture(window: window)
        XCTAssertNil(captured)
    }
}

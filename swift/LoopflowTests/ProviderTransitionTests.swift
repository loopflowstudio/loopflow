import Foundation
import Testing
import SwiftUI
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@MainActor
struct ProviderTransitionTests {
    @Test func theConnectionRowNamesTheObservedForkAndDatesSourceLiveness() throws {
        let path = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("tests/fixtures/dto/provider_transition.json")
        let transition = try JSONDecoder().decode(ProviderTransition.self, from: Data(contentsOf: path))
        let row = ProviderTransitionRow(transition: transition)
        #expect(try row.inspect().find(text: "Forked from earlier; original was running at fork").string() == transition.explanation)
        let reading = ServerConnectionReading(generation: 2, state: .connected, input: false, nativeReady: true, message: nil, transition: transition)
        #expect(reading.title?.contains("Read-only") == true)
        #expect(!reading.acceptsInput)
    }
}

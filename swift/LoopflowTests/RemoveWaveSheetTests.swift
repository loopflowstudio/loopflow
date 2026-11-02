#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Remove Wave sheet")
@MainActor
struct RemoveWaveSheetTests {
    private func preview() throws -> WaveRemoval {
        try RegistryQuery.decode(
            WaveRemoval.self, from: String(decoding: loadFixtureData("wave_removal.json"), as: UTF8.self))
    }

    @Test func previewRendersRustSentencesAndEnablesRemove() throws {
        let removal = try preview()
        var removed = false
        let view = RemoveWaveSheetContent(
            name: "desktop", phase: .preview(removal),
            onRemove: { removed = true }, onRetry: {}, onCancel: {})
        let summary = try view.inspect().find(viewWithAccessibilityIdentifier: "remove-wave-summary")
        #expect(try summary.find(text: removal.summary[0]).string() == removal.summary[0])
        try view.inspect().find(viewWithAccessibilityIdentifier: "remove-wave-confirm").button().tap()
        #expect(removed)
    }

    @Test func failureKeepsThePreviewAndOffersRetry() throws {
        var retried = false
        let view = RemoveWaveSheetContent(
            name: "desktop",
            phase: .failed(preview: try preview(), message: "1 of 2 open Tasks moved; LOO-2 could not"),
            onRemove: {}, onRetry: { retried = true }, onCancel: {})
        #expect(try view.inspect().find(viewWithAccessibilityIdentifier: "remove-wave-error").text().string()
            == "1 of 2 open Tasks moved; LOO-2 could not")
        #expect(throws: (any Error).self) {
            try view.inspect().find(viewWithAccessibilityIdentifier: "remove-wave-confirm")
        }
        try view.inspect().find(viewWithAccessibilityIdentifier: "remove-wave-retry").button().tap()
        #expect(retried)
    }
}
#endif

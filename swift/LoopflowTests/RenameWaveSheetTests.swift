#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Rename Wave sheet")
@MainActor
struct RenameWaveSheetTests {
    private func renamed() throws -> WaveRenamed {
        try RegistryQuery.decode(
            WaveRenamed.self, from: String(decoding: loadFixtureData("wave_renamed.json"), as: UTF8.self))
    }

    @Test func startsFromTheCurrentAddressWithRenameDisabled() throws {
        let view = RenameWaveSheet(
            model: WorkModel(query: RegistryQuery { _, _ in "{}" }), id: "wave-id",
            currentName: "product/desktop", repo: "/repo", onDone: {})
        #expect(try view.inspect().find(text: "Rename product/desktop").string() == "Rename product/desktop")
        #expect(try view.inspect().find(viewWithAccessibilityIdentifier: "rename-wave-confirm").button().isDisabled())
    }

    @Test func leftoversStayOpenWithTheRustLinesAndADoneButton() throws {
        let receipt = try renamed()
        var done = false
        let view = RenameWaveSheetContent(
            currentName: "product", newName: .constant("prod"), phase: .leftovers(receipt),
            onRename: {}, onDone: { done = true })
        let summary = try view.inspect().find(viewWithAccessibilityIdentifier: "rename-wave-summary")
        #expect(try summary.find(text: receipt.summary[0]).string() == receipt.summary[0])
        #expect(throws: (any Error).self) {
            try view.inspect().find(viewWithAccessibilityIdentifier: "rename-wave-confirm")
        }
        try view.inspect().find(viewWithAccessibilityIdentifier: "rename-wave-done").button().tap()
        #expect(done)
    }

    @Test func failureKeepsTheNameAndShowsTheError() throws {
        var renamed = false
        let view = RenameWaveSheetContent(
            currentName: "product", newName: .constant("ops"), phase: .editing(error: "ops already exists"),
            onRename: { renamed = true }, onDone: {})
        #expect(try view.inspect().find(viewWithAccessibilityIdentifier: "rename-wave-error").text().string()
            == "ops already exists")
        try view.inspect().find(viewWithAccessibilityIdentifier: "rename-wave-confirm").button().tap()
        #expect(renamed)
    }
}
#endif

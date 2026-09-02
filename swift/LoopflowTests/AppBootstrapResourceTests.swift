import Foundation
import Testing
@testable import Loopflow

@Suite("App Bootstrap Resources")
struct AppBootstrapResourceTests {
    @Test("packaged app resolves the SwiftPM bundle from Contents/Resources")
    func packagedBundleResolvesFromResources() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let resources = root.appendingPathComponent("Contents/Resources", isDirectory: true)
        let resourceBundle = resources.appendingPathComponent(
            "LoopflowSwift_Loopflow.bundle",
            isDirectory: true
        )
        try FileManager.default.createDirectory(
            at: resourceBundle.appendingPathComponent("Fonts", isDirectory: true),
            withIntermediateDirectories: true
        )
        defer { try? FileManager.default.removeItem(at: root) }

        let resolved = Bundle.packagedLoopflowResources(at: resources)

        #expect(resolved?.bundleURL.standardizedFileURL == resourceBundle.standardizedFileURL)
    }
}

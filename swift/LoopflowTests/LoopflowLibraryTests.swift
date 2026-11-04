#if os(macOS)
import Foundation
import Testing
@testable import LoopflowMac
import Loopflow

@Suite("Loopflow library")
struct LoopflowLibraryTests {
    /// A write parses the whole `lf` command line, whose debug-build frames
    /// are larger than a Swift task thread's stack.
    @Test func writeRunsOnAStackLargeEnoughToParseItsCommand() async throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        await #expect(throws: RegistryQueryError.self) {
            try await LoopflowLibrary.write(.taskReopen(directory: directory.path, issue: "NOPE-0"))
        }
    }
}
#endif

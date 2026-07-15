import Foundation
import Testing

@testable import Loopflow
@testable import LoopflowMac

@Suite("Wave detail reading")
struct WaveDetailReadingTests {
    @Test("a failed refresh preserves the last successful Wave detail")
    func failedRefreshPreservesLastGoodDetail() throws {
        let detail = try JSONDecoder().decode(
            WaveDetailSnapshot.self,
            from: loadFixtureData("wave_detail.json")
        )
        var reading = WaveDetailReading()

        reading.update(detail)
        reading.recordFailure(RegistryQueryError("registry busy"))

        #expect(reading.snapshot?.wave.id == "wave-1")
        #expect(reading.snapshot?.projects[0].project.slug == "release-feedback")
        #expect(reading.errorMessage == "Wave status unavailable: registry busy")
    }

    @Test("a successful refresh clears the stale warning")
    func successfulRefreshClearsWarning() throws {
        let detail = try JSONDecoder().decode(
            WaveDetailSnapshot.self,
            from: loadFixtureData("wave_detail.json")
        )
        var reading = WaveDetailReading()
        reading.recordFailure(RegistryQueryError("registry busy"))

        reading.update(detail)

        #expect(reading.errorMessage == nil)
        #expect(reading.snapshot?.wave.id == "wave-1")
    }

    private func loadFixtureData(_ name: String, sourceFile: String = #filePath) throws -> Data {
        let testFile = URL(fileURLWithPath: sourceFile)
        let fixtures = testFile
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto")
            .appendingPathComponent(name)
        return try Data(contentsOf: fixtures)
    }
}

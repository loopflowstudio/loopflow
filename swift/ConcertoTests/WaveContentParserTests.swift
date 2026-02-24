import Foundation
import Testing
@testable import LoopflowCore

@Suite("WaveContentParser")
struct WaveContentParserTests {
    @Test("parses README sections and roadmap progress")
    func parsesSectionsAndRoadmap() throws {
        let repoRoot = try makeTempRepo()
        defer { try? FileManager.default.removeItem(at: repoRoot) }

        let waveDir = repoRoot
            .appendingPathComponent("wave", isDirectory: true)
            .appendingPathComponent("demo-wave", isDirectory: true)
        try FileManager.default.createDirectory(at: waveDir, withIntermediateDirectories: true)

        let readme = """
        # Demo

        ## Vision
        Build a delightful onboarding.
        ### Not here
        Keep this subsection with vision.

        ## Goals
        - Launch design from Concerto
        - Show roadmap status

        ## Risks
        - Parser drift

        ## Metrics
        - Time to first wave

        ## Notes
        Ignore this section.
        """
        try readme.write(to: waveDir.appendingPathComponent("README.md"), atomically: true, encoding: .utf8)

        let roadmapOne = """
        # Design-first entry

        ## Shipped
        Done.
        """
        try roadmapOne.write(to: waveDir.appendingPathComponent("01-design-entry.md"), atomically: true, encoding: .utf8)

        let roadmapTwo = """
        # Detail panel content

        ## Plan
        In progress.
        """
        try roadmapTwo.write(to: waveDir.appendingPathComponent("02-detail-panel.md"), atomically: true, encoding: .utf8)

        let content = WaveContentParser.parse(repoRoot: repoRoot, waveName: "demo-wave")

        #expect(content?.vision?.contains("Build a delightful onboarding.") == true)
        #expect(content?.vision?.contains("### Not here") == true)
        #expect(content?.goals?.contains("Launch design from Concerto") == true)
        #expect(content?.risks == "- Parser drift")
        #expect(content?.metrics == "- Time to first wave")

        #expect(content?.roadmapItems.count == 2)
        #expect(content?.roadmapItems[0].id == "01-design-entry")
        #expect(content?.roadmapItems[0].title == "Design-first entry")
        #expect(content?.roadmapItems[0].isShipped == true)
        #expect(content?.roadmapItems[1].id == "02-detail-panel")
        #expect(content?.roadmapItems[1].isShipped == false)
    }

    @Test("returns nil when wave has no README sections or roadmap")
    func returnsNilForEmptyWaveContent() throws {
        let repoRoot = try makeTempRepo()
        defer { try? FileManager.default.removeItem(at: repoRoot) }

        let waveDir = repoRoot
            .appendingPathComponent("wave", isDirectory: true)
            .appendingPathComponent("empty-wave", isDirectory: true)
        try FileManager.default.createDirectory(at: waveDir, withIntermediateDirectories: true)

        try "# Notes only".write(
            to: waveDir.appendingPathComponent("README.md"),
            atomically: true,
            encoding: .utf8
        )

        let content = WaveContentParser.parse(repoRoot: repoRoot, waveName: "empty-wave")
        #expect(content == nil)
    }

    private func makeTempRepo() throws -> URL {
        let repoRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("loopflow-wave-content-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: repoRoot, withIntermediateDirectories: true)
        return repoRoot
    }
}

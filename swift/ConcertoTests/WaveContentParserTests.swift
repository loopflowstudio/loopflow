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

        ## Strategy
        Start with the simplest flow that works.

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
        #expect(content?.strategy == "Start with the simplest flow that works.")
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

    @Test("parses roadmap item content for non-shipped items")
    func parsesRoadmapItemContent() throws {
        let repoRoot = try makeTempRepo()
        defer { try? FileManager.default.removeItem(at: repoRoot) }

        let waveDir = repoRoot
            .appendingPathComponent("wave", isDirectory: true)
            .appendingPathComponent("content-wave", isDirectory: true)
        try FileManager.default.createDirectory(at: waveDir, withIntermediateDirectories: true)

        try "# Minimal".write(
            to: waveDir.appendingPathComponent("README.md"),
            atomically: true,
            encoding: .utf8
        )

        let roadmapWithContent = """
        # Feature Alpha

        This is the first paragraph explaining the feature.
        It has multiple lines of detail.

        ## Plan
        - Step one
        - Step two
        """
        try roadmapWithContent.write(
            to: waveDir.appendingPathComponent("01-feature-alpha.md"),
            atomically: true,
            encoding: .utf8
        )

        let shippedItem = """
        # Shipped Feature

        This content should not be included.

        ## Shipped
        Done.
        """
        try shippedItem.write(
            to: waveDir.appendingPathComponent("02-shipped-feature.md"),
            atomically: true,
            encoding: .utf8
        )

        let content = WaveContentParser.parse(repoRoot: repoRoot, waveName: "content-wave")

        #expect(content?.roadmapItems.count == 2)

        let first = content?.roadmapItems[0]
        #expect(first?.title == "Feature Alpha")
        #expect(first?.content?.contains("first paragraph") == true)
        #expect(first?.filePath != nil)

        let second = content?.roadmapItems[1]
        #expect(second?.title == "Shipped Feature")
        #expect(second?.isShipped == true)
        #expect(second?.content == nil)
    }

    @Test("parses scratch doc when branch is provided")
    func parsesScratchDoc() throws {
        let repoRoot = try makeTempRepo()
        defer { try? FileManager.default.removeItem(at: repoRoot) }

        let waveDir = repoRoot
            .appendingPathComponent("wave", isDirectory: true)
            .appendingPathComponent("scratch-wave", isDirectory: true)
        try FileManager.default.createDirectory(at: waveDir, withIntermediateDirectories: true)

        try "## Goals\n- Test goal".write(
            to: waveDir.appendingPathComponent("README.md"),
            atomically: true,
            encoding: .utf8
        )

        let scratchDir = repoRoot.appendingPathComponent("scratch", isDirectory: true)
        try FileManager.default.createDirectory(at: scratchDir, withIntermediateDirectories: true)

        let scratchContent = """
        # Design Doc

        This is the design document for the feature.
        """
        try scratchContent.write(
            to: scratchDir.appendingPathComponent("feature-branch.md"),
            atomically: true,
            encoding: .utf8
        )

        // Without branch — no scratch doc
        let withoutBranch = WaveContentParser.parse(repoRoot: repoRoot, waveName: "scratch-wave")
        #expect(withoutBranch?.scratchDoc == nil)

        // With branch — scratch doc found
        let withBranch = WaveContentParser.parse(repoRoot: repoRoot, waveName: "scratch-wave", branch: "feature-branch")
        #expect(withBranch?.scratchDoc?.contains("Design Doc") == true)
        #expect(withBranch?.scratchDocPath?.contains("scratch/feature-branch.md") == true)
    }

    @Test("returns content when only scratch doc exists")
    func returnsContentForScratchDocOnly() throws {
        let repoRoot = try makeTempRepo()
        defer { try? FileManager.default.removeItem(at: repoRoot) }

        let waveDir = repoRoot
            .appendingPathComponent("wave", isDirectory: true)
            .appendingPathComponent("doc-only", isDirectory: true)
        try FileManager.default.createDirectory(at: waveDir, withIntermediateDirectories: true)

        try "# Notes only".write(
            to: waveDir.appendingPathComponent("README.md"),
            atomically: true,
            encoding: .utf8
        )

        let scratchDir = repoRoot.appendingPathComponent("scratch", isDirectory: true)
        try FileManager.default.createDirectory(at: scratchDir, withIntermediateDirectories: true)
        try "Design content".write(
            to: scratchDir.appendingPathComponent("my-branch.md"),
            atomically: true,
            encoding: .utf8
        )

        // Without branch: no roadmap, no sections -> nil
        let without = WaveContentParser.parse(repoRoot: repoRoot, waveName: "doc-only")
        #expect(without == nil)

        // With branch: scratch doc found -> non-nil
        let with = WaveContentParser.parse(repoRoot: repoRoot, waveName: "doc-only", branch: "my-branch")
        #expect(with != nil)
        #expect(with?.scratchDoc == "Design content")
    }

    private func makeTempRepo() throws -> URL {
        let repoRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("loopflow-wave-content-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: repoRoot, withIntermediateDirectories: true)
        return repoRoot
    }
}

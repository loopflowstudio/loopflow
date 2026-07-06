import Foundation
import Testing
@testable import LoopflowCore

@Suite("WaveContentParser")
struct WaveContentParserTests {
    @Test("parses README sections")
    func parsesSections() throws {
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
        - Show current wave status

        ## Risks
        - Parser drift

        ## Metrics
        - Time to first wave

        ## Notes
        Ignore this section.
        """
        try readme.write(to: waveDir.appendingPathComponent("README.md"), atomically: true, encoding: .utf8)

        let content = WaveContentParser.parse(repoRoot: repoRoot, waveName: "demo-wave")

        #expect(content?.vision?.contains("Build a delightful onboarding.") == true)
        #expect(content?.vision?.contains("### Not here") == true)
        #expect(content?.strategy == "Start with the simplest flow that works.")
        #expect(content?.goals?.contains("Launch design from Concerto") == true)
        #expect(content?.risks == "- Parser drift")
        #expect(content?.metrics == "- Time to first wave")
    }

    @Test("uses the first README paragraph as vision when present")
    func usesLeadingParagraphAsVision() throws {
        let repoRoot = try makeTempRepo()
        defer { try? FileManager.default.removeItem(at: repoRoot) }

        let waveDir = repoRoot
            .appendingPathComponent("wave", isDirectory: true)
            .appendingPathComponent("tagline-wave", isDirectory: true)
        try FileManager.default.createDirectory(at: waveDir, withIntermediateDirectories: true)

        let readme = """
        # Tagline Wave

        Agents that remember.
        Work that compounds.

        ## Strategy
        Start with the README.
        """
        try readme.write(to: waveDir.appendingPathComponent("README.md"), atomically: true, encoding: .utf8)

        let content = try #require(WaveContentParser.parse(repoRoot: repoRoot, waveName: "tagline-wave"))

        #expect(content.vision == "Agents that remember.\nWork that compounds.")
        #expect(content.strategy == "Start with the README.")
    }

    @Test("skips leading README headings before the tagline paragraph")
    func skipsLeadingHeadingsBeforeVisionParagraph() throws {
        let repoRoot = try makeTempRepo()
        defer { try? FileManager.default.removeItem(at: repoRoot) }

        let waveDir = repoRoot
            .appendingPathComponent("wave", isDirectory: true)
            .appendingPathComponent("heading-wave", isDirectory: true)
        try FileManager.default.createDirectory(at: waveDir, withIntermediateDirectories: true)

        let readme = """
        # Heading Wave

        ## Overview

        Agents that remember.
        Work that compounds.

        ## Vision
        Legacy fallback.
        """
        try readme.write(to: waveDir.appendingPathComponent("README.md"), atomically: true, encoding: .utf8)

        let content = try #require(WaveContentParser.parse(repoRoot: repoRoot, waveName: "heading-wave"))

        #expect(content.vision == "Agents that remember.\nWork that compounds.")
    }

    @Test("returns nil when wave has no README sections")
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

    @Test("ignores numbered markdown files")
    func ignoresNumberedMarkdownFiles() throws {
        let repoRoot = try makeTempRepo()
        defer { try? FileManager.default.removeItem(at: repoRoot) }

        let waveDir = repoRoot
            .appendingPathComponent("wave", isDirectory: true)
            .appendingPathComponent("content-wave", isDirectory: true)
        try FileManager.default.createDirectory(at: waveDir, withIntermediateDirectories: true)

        try "# Notes only".write(
            to: waveDir.appendingPathComponent("README.md"),
            atomically: true,
            encoding: .utf8
        )

        let numberedFile = """
        # Feature Alpha

        This is the first paragraph explaining the feature.
        It has multiple lines of detail.

        ## Plan
        - Step one
        - Step two
        """
        try numberedFile.write(
            to: waveDir.appendingPathComponent("01-feature-alpha.md"),
            atomically: true,
            encoding: .utf8
        )

        let content = WaveContentParser.parse(repoRoot: repoRoot, waveName: "content-wave")
        #expect(content == nil)
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

        // Without branch: no sections -> nil
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

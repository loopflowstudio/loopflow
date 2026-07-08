import Foundation
import Testing
@testable import Loopflow

@Suite("WavePlanParser")
struct WavePlanParserTests {
    @Test("parses objective and project KRs")
    func parsesObjectiveAndProjects() throws {
        let repoRoot = try makeTempRepo()
        defer { try? FileManager.default.removeItem(at: repoRoot) }

        let waveDir = repoRoot
            .appendingPathComponent("wave", isDirectory: true)
            .appendingPathComponent("loopflow", isDirectory: true)
        let projectsDir = waveDir.appendingPathComponent("projects", isDirectory: true)
        try FileManager.default.createDirectory(at: projectsDir, withIntermediateDirectories: true)

        let goal = """
        ---
        crons: []
        ---

        ## Objective

        Make Loopflow the daily surface.
        Frame, don't render.

        ## Projects

        Read `projects/`.
        """
        try goal.write(to: waveDir.appendingPathComponent("GOAL.md"), atomically: true, encoding: .utf8)

        let project = """
        # Session lifecycle

        The spine. A wave session must outlive the app.

        ## KRs

        - A running wave session survives app restart and reattaches cleanly in 5/5
          dogfood trials.
        - Launching or attaching takes one action.

        ## Notes

        Ignore this section.
        """
        try project.write(to: projectsDir.appendingPathComponent("session-lifecycle.md"), atomically: true, encoding: .utf8)

        let plan = try #require(WavePlanParser.parse(repoRoot: repoRoot, waveName: "loopflow"))

        #expect(plan.objective == "Make Loopflow the daily surface.\nFrame, don't render.")
        #expect(plan.projects.count == 1)
        #expect(plan.projects[0].id == "session-lifecycle")
        #expect(plan.projects[0].title == "Session lifecycle")
        #expect(plan.projects[0].summary == "The spine. A wave session must outlive the app.")
        #expect(plan.projects[0].krs == [
            "A running wave session survives app restart and reattaches cleanly in 5/5 dogfood trials.",
            "Launching or attaching takes one action.",
        ])
    }

    @Test("sorts projects by filename")
    func sortsProjectsByFilename() throws {
        let repoRoot = try makeTempRepo()
        defer { try? FileManager.default.removeItem(at: repoRoot) }

        let projectsDir = repoRoot
            .appendingPathComponent("wave", isDirectory: true)
            .appendingPathComponent("loopflow", isDirectory: true)
            .appendingPathComponent("projects", isDirectory: true)
        try FileManager.default.createDirectory(at: projectsDir, withIntermediateDirectories: true)

        try "# Wave conducting".write(
            to: projectsDir.appendingPathComponent("wave-conducting.md"),
            atomically: true,
            encoding: .utf8
        )
        try "# Attention".write(
            to: projectsDir.appendingPathComponent("attention-navigation.md"),
            atomically: true,
            encoding: .utf8
        )

        let plan = try #require(WavePlanParser.parse(repoRoot: repoRoot, waveName: "loopflow"))

        #expect(plan.objective.isEmpty)
        #expect(plan.projects.map(\.id) == ["attention-navigation", "wave-conducting"])
    }

    @Test("returns nil for missing local plan")
    func returnsNilForMissingPlan() throws {
        let repoRoot = try makeTempRepo()
        defer { try? FileManager.default.removeItem(at: repoRoot) }

        let plan = WavePlanParser.parse(repoRoot: repoRoot, waveName: "loopflow")

        #expect(plan == nil)
    }

    private func makeTempRepo() throws -> URL {
        let repoRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("loopflow-wave-plan-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: repoRoot, withIntermediateDirectories: true)
        return repoRoot
    }
}

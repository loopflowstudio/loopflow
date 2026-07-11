import Foundation
import Testing
@testable import Loopflow

@Suite("WavePlanParser")
struct WavePlanParserTests {
    @Test("parses the objective from GOAL.md")
    func parsesObjective() throws {
        let repoRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("loopflow-wave-plan-tests-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: repoRoot) }
        let waveDir = repoRoot.appendingPathComponent("wave/loopflow", isDirectory: true)
        try FileManager.default.createDirectory(at: waveDir, withIntermediateDirectories: true)
        try """
        ---
        crons: []
        ---

        ## Objective

        Make Loopflow the daily surface.
        Frame, don't render.

        ## Bounds

        Stay small.
        """.write(
            to: waveDir.appendingPathComponent("GOAL.md"),
            atomically: true,
            encoding: .utf8
        )

        #expect(
            WavePlanParser.objective(repoRoot: repoRoot, waveName: "loopflow")
                == "Make Loopflow the daily surface.\nFrame, don't render."
        )
    }

    @Test("returns nil when GOAL.md is absent")
    func missingGoal() {
        #expect(
            WavePlanParser.objective(
                repoRoot: FileManager.default.temporaryDirectory,
                waveName: "missing"
            ) == nil
        )
    }
}

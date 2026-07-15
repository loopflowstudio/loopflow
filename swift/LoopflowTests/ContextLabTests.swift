import Foundation
import Loopflow
import Testing

@testable import LoopflowMac

@Suite("Context Lab")
struct ContextLabTests {
    @Test("Revision comparison waits for enough similarly captured evidence")
    func revisionComparisonRequiresComparablePopulations() {
        #expect(contextRevisionComparisonBlocker(
            earlierLaunches: 2,
            earlierCompleteCaptures: 2,
            laterLaunches: 3,
            laterCompleteCaptures: 3
        ) != nil)
        #expect(contextRevisionComparisonBlocker(
            earlierLaunches: 10,
            earlierCompleteCaptures: 10,
            laterLaunches: 10,
            laterCompleteCaptures: 8
        ) != nil)
        #expect(contextRevisionComparisonBlocker(
            earlierLaunches: 5,
            earlierCompleteCaptures: 4,
            laterLaunches: 10,
            laterCompleteCaptures: 7
        ) == nil)
    }

    @Test("Task workspace backlinks retain the exact research selection")
    func contextBacklinkRoundTrips() throws {
        let query = SessionSetQuery(
            repoPaths: ["/src/loopflow"],
            startedAfter: 10,
            startedBefore: 20,
            waves: ["intelligence"],
            flows: [],
            skills: ["implement"],
            providers: ["codex"],
            models: [],
            surfaces: ["tui"],
            outcomes: [.completed],
            captureStates: [.complete]
        )
        let route = TaskWorkspaceRoute(
            wave: "intelligence",
            issue: "INT-42",
            repoPath: "/src/loopflow",
            context: ContextLabRoute(
                query: query,
                selectedNodeId: "revision-1",
                focusNodeId: "source-1",
                mode: .lanes
            )
        )

        let decoded = try JSONDecoder().decode(
            TaskWorkspaceRoute.self,
            from: JSONEncoder().encode(route)
        )

        #expect(decoded == route)
        #expect(decoded.context.query.startedAfter == 10)
        #expect(decoded.context.selectedNodeId == "revision-1")
    }

    @Test("Task refinement maps the canonical repo file into the Task worktree")
    func taskSourceMapsIntoWorktree() throws {
        let path = try taskSourcePath(
            sourcePath: "/src/loopflow/rust/loopflow/src/engine/builtins/LOOPFLOW.md",
            repoPath: "/src/loopflow",
            worktree: "/src/loopflow.intelligence.context"
        )

        #expect(path == "/src/loopflow.intelligence.context/rust/loopflow/src/engine/builtins/LOOPFLOW.md")
        #expect(throws: ContextRefinementError.self) {
            _ = try taskSourcePath(
                sourcePath: "/tmp/unrelated.md",
                repoPath: "/src/loopflow",
                worktree: "/src/loopflow.intelligence.context"
            )
        }
    }

    @Test("Stale-hash checks use the effective prompt text")
    func effectiveSourceHashesMatchCapturedSlices() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("context-lab-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        let operating = directory.appendingPathComponent("LOOPFLOW.md")
        try "Guide\n".write(to: operating, atomically: true, encoding: .utf8)
        #expect(
            effectiveSourceHash(kind: "operating_instructions", path: operating.path)
                == "b779142188232028405d7f6245309de84876fde2d27c0df5c0c807bab73194ae"
        )

        let skill = directory.appendingPathComponent("refine.md")
        try "---\nname: refine\n---\nBuild it.\n".write(to: skill, atomically: true, encoding: .utf8)
        #expect(
            effectiveSourceHash(kind: "skill_instructions", path: skill.path)
                == "0c2558221675df997f23b3c68d5cac2fb9b18a875ab1eeb092f7780bb320a7a3"
        )
    }
}

import Foundation
import Loopflow
import Testing

@testable import LoopflowMac

@Suite("Context Lab")
struct ContextLabTests {
    @Test("Context Lab menu routes open the current repo over 30 days")
    func initialContextLabRouteIsExplicit() {
        let route = ContextLabRoute.initial(repoPath: "/src/loopflow", now: 3_000_000)

        #expect(route.query.repoPaths == ["/src/loopflow"])
        #expect(route.query.startedAfter == 408_000)
        #expect(route.query.startedBefore == 3_000_000)
        #expect(route.selectedNodeId == "session-set")
        #expect(route.focusNodeId == "session-set")
        #expect(route.mode == .aggregate)
    }

    @Test("Saved visualization modes use semantic values")
    func contextLabModesPersistIndependentlyFromLabels() {
        #expect(ContextLabMode.allCases.map(\.rawValue) == ["aggregate", "lanes", "table"])
        #expect(ContextLabMode.allCases.map(\.title) == ["Aggregate flame", "Session lanes", "Table"])
    }

    @Test("Selected-source sorting uses share rather than raw token load")
    func selectedSourceShareUsesTheSessionDenominator() {
        let heavierSlice = contextSelectedSourceShare(
            selectedTokens: 600,
            contextTokens: 2_000
        )
        let largerShare = contextSelectedSourceShare(
            selectedTokens: 500,
            contextTokens: 1_000
        )

        #expect(largerShare > heavierSlice)
        #expect(contextSelectedSourceShare(selectedTokens: 10, contextTokens: 0) == 0)
    }

    @Test("Revision comparison waits for enough similarly captured evidence")
    func revisionComparisonRequiresComparablePopulations() {
        let fiveCodex = [ProviderModelExposure(provider: "codex", model: "gpt-5", exposedLaunches: 5)]
        let tenCodex = [ProviderModelExposure(provider: "codex", model: "gpt-5", exposedLaunches: 10)]
        #expect(contextRevisionComparisonBlocker(
            earlierLaunches: 2,
            earlierCompleteCaptures: 2,
            laterLaunches: 3,
            laterCompleteCaptures: 3,
            earlierProviderModels: [],
            laterProviderModels: [],
            earlierFirstSeen: nil,
            earlierLastSeen: nil,
            laterFirstSeen: nil,
            laterLastSeen: nil
        ) != nil)
        #expect(contextRevisionComparisonBlocker(
            earlierLaunches: 10,
            earlierCompleteCaptures: 10,
            laterLaunches: 10,
            laterCompleteCaptures: 8,
            earlierProviderModels: tenCodex,
            laterProviderModels: tenCodex,
            earlierFirstSeen: 100,
            earlierLastSeen: 200,
            laterFirstSeen: 300,
            laterLastSeen: 400
        ) != nil)
        #expect(contextRevisionComparisonBlocker(
            earlierLaunches: 5,
            earlierCompleteCaptures: 4,
            laterLaunches: 10,
            laterCompleteCaptures: 7,
            earlierProviderModels: fiveCodex,
            laterProviderModels: tenCodex,
            earlierFirstSeen: 100,
            earlierLastSeen: 200,
            laterFirstSeen: 300,
            laterLastSeen: 450
        ) == nil)
    }

    @Test("Revision comparison balances provider mix and observation spans")
    func revisionComparisonRejectsConfoundedPopulations() {
        let codex = [ProviderModelExposure(provider: "codex", model: "gpt-5", exposedLaunches: 10)]
        let claude = [ProviderModelExposure(provider: "claude", model: nil, exposedLaunches: 10)]
        #expect(contextRevisionComparisonBlocker(
            earlierLaunches: 10,
            earlierCompleteCaptures: 9,
            laterLaunches: 10,
            laterCompleteCaptures: 9,
            earlierProviderModels: codex,
            laterProviderModels: claude,
            earlierFirstSeen: 100,
            earlierLastSeen: 200,
            laterFirstSeen: 300,
            laterLastSeen: 400
        )?.contains("provider/model mix") == true)
        #expect(contextRevisionComparisonBlocker(
            earlierLaunches: 10,
            earlierCompleteCaptures: 9,
            laterLaunches: 10,
            laterCompleteCaptures: 9,
            earlierProviderModels: codex,
            laterProviderModels: codex,
            earlierFirstSeen: 100,
            earlierLastSeen: 200,
            laterFirstSeen: 300,
            laterLastSeen: 550
        )?.contains("observation spans") == true)
        #expect(contextRevisionComparisonBlocker(
            earlierLaunches: 10,
            earlierCompleteCaptures: 9,
            laterLaunches: 10,
            laterCompleteCaptures: 9,
            earlierProviderModels: [],
            laterProviderModels: codex,
            earlierFirstSeen: 100,
            earlierLastSeen: 200,
            laterFirstSeen: 300,
            laterLastSeen: 400
        )?.contains("exposure is missing") == true)
    }

    @Test("Task workspace backlinks retain the exact research selection")
    func contextBacklinkRoundTrips() throws {
        let query = SessionSetQuery(
            repoPaths: ["/src/loopflow"],
            startedAfter: 10,
            startedBefore: 20,
            waves: ["intelligence"],
            projects: ["context"],
            tasks: ["W2-71"],
            flows: [],
            skills: ["implement"],
            providers: ["codex"],
            models: [],
            surfaces: ["tui"],
            outcomes: [.completed],
            captureStates: [.complete],
            steeredOnly: true,
            currentRevisionOnly: true
        )
        let route = TaskWorkspaceRoute(
            wave: "intelligence",
            issue: "INT-42",
            repoPath: "/src/loopflow",
            initialSection: .terminal,
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
        #expect(decoded.context.query.steeredOnly)
        #expect(decoded.context.query.currentRevisionOnly)
        #expect(decoded.context.selectedNodeId == "revision-1")
        #expect(decoded.initialSection == .terminal)
    }

    @Test("Refinement rechecks an idle Task Session and its worktree")
    func refinementTaskChoiceMustRemainCurrent() throws {
        let testFile = URL(fileURLWithPath: #filePath)
        let fixture = testFile
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")
        let roadmap = try JSONDecoder().decode(
            RoadmapSnapshot.self,
            from: Data(contentsOf: fixture)
        )
        let query = SessionSetQuery(
            repoPaths: ["/src/loopflow"],
            startedAfter: 10,
            startedBefore: 20,
            waves: [],
            projects: [],
            tasks: [],
            flows: [],
            skills: [],
            providers: [],
            models: [],
            surfaces: [],
            outcomes: [],
            captureStates: [],
            steeredOnly: false,
            currentRevisionOnly: false
        )

        let choices = refinementTaskChoices(in: roadmap, query: query)
        let choice = try #require(choices.first)

        #expect(choices.map(\.id) == ["W2-131"])
        #expect(refinementWorkspaceIsCurrent(choice, current: choice))
        let moved = RefinementTaskChoice(
            waveId: "wave-2",
            waveName: choice.waveName,
            repoPath: choice.repoPath,
            task: choice.task
        )
        #expect(!refinementWorkspaceIsCurrent(choice, current: moved))
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

    @Test("Stale-hash checks compare exact source bytes")
    func sourceFileHashesMatchRustEvidence() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("context-lab-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }

        let operating = directory.appendingPathComponent("LOOPFLOW.md")
        try "Guide\n".write(to: operating, atomically: true, encoding: .utf8)
        let operatingReceipt = try #require(sourceFileHash(path: operating.path))
        #expect(operatingReceipt == "3274fcad886cde4e2ca86b11d30fd7c44858eadf1c437a9583f31e7815db1af6")

        try "Changed\n".write(to: operating, atomically: true, encoding: .utf8)
        #expect(sourceFileHash(path: operating.path) != operatingReceipt)

        let skill = directory.appendingPathComponent("refine.md")
        try "---\nname: refine\n---\nBuild it.\n".write(to: skill, atomically: true, encoding: .utf8)
        #expect(
            sourceFileHash(path: skill.path)
                == "2540f0cd5850caabe224a6419fa2accb3d5e7e06eae02b5ce953d14244496ee3"
        )
    }
}

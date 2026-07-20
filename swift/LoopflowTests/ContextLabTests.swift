import Foundation
import Loopflow
import Testing

@testable import LoopflowMac

@Suite("Context Lab")
struct ContextLabTests {
    @Test("Wave routes bind Context Lab to one repo and Wave over 30 days")
    func waveContextLabRouteIsExplicit() {
        let route = ContextLabRoute.wave(
            repoPath: "/src/loopflow",
            wave: "product",
            now: 3_000_000
        )

        #expect(route.query.repoPaths == ["/src/loopflow"])
        #expect(route.query.waves == ["product"])
        #expect(route.query.startedAfter == 408_000)
        #expect(route.query.startedBefore == 3_000_000)
        #expect(route.selectedNodeId == "invocation-set")
        #expect(route.focusNodeId == "invocation-set")
        #expect(route.mode == .aggregate)
        #expect(route.isWaveScoped)

        var unscopedQuery = route.query
        unscopedQuery.waves = []
        #expect(!ContextLabRoute(
            query: unscopedQuery,
            selectedNodeId: route.selectedNodeId,
            focusNodeId: route.focusNodeId,
            mode: route.mode
        ).isWaveScoped)
    }

    @Test("Saved visualization modes use semantic values")
    func contextLabModesPersistIndependentlyFromLabels() {
        #expect(ContextLabMode.allCases.map(\.rawValue) == ["aggregate", "lanes", "sources"])
        #expect(ContextLabMode.allCases.map(\.title) == ["Initial prompts", "Invocations", "Sources"])
    }

    @Test("Source refinement stays inside the selected canonical repo")
    func sourceRefinementUsesTheCanonicalRepoBoundary() {
        let query = ContextLabRoute.wave(
            repoPath: "/src/loopflow",
            wave: "product",
            now: 3_000_000
        ).query

        #expect(contextRelativeSourcePath(
            "/src/loopflow/rust/loopflow/src/engine/builtins/task/skill/refine.md",
            repoPath: query.repoPaths[0]
        ) == "rust/loopflow/src/engine/builtins/task/skill/refine.md")
        #expect(contextRelativeSourcePath(
            "/src/loopflow-other/refine.md",
            repoPath: query.repoPaths[0]
        ) == nil)
        #expect(contextRelativeSourcePath(
            #filePath,
            repoPath: URL(fileURLWithPath: #filePath)
                .deletingLastPathComponent()
                .deletingLastPathComponent()
                .deletingLastPathComponent()
                .path
        ) == "swift/LoopflowTests/ContextLabTests.swift")
    }

    @Test("Selected-source sorting uses share rather than raw token load")
    func selectedSourceShareUsesTheLaunchDenominator() {
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
        let fiveCodex = [ProviderModelExposure(provider: "codex", model: "gpt-5", exposedInvocations: 5)]
        let tenCodex = [ProviderModelExposure(provider: "codex", model: "gpt-5", exposedInvocations: 10)]
        #expect(contextRevisionComparisonBlocker(
            earlierInvocations: 2,
            earlierCompleteCaptures: 2,
            laterInvocations: 3,
            laterCompleteCaptures: 3,
            earlierProviderModels: [],
            laterProviderModels: [],
            earlierFirstSeen: nil,
            earlierLastSeen: nil,
            laterFirstSeen: nil,
            laterLastSeen: nil
        ) != nil)
        #expect(contextRevisionComparisonBlocker(
            earlierInvocations: 10,
            earlierCompleteCaptures: 10,
            laterInvocations: 10,
            laterCompleteCaptures: 8,
            earlierProviderModels: tenCodex,
            laterProviderModels: tenCodex,
            earlierFirstSeen: 100,
            earlierLastSeen: 200,
            laterFirstSeen: 300,
            laterLastSeen: 400
        ) != nil)
        #expect(contextRevisionComparisonBlocker(
            earlierInvocations: 5,
            earlierCompleteCaptures: 4,
            laterInvocations: 10,
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
        let codex = [ProviderModelExposure(provider: "codex", model: "gpt-5", exposedInvocations: 10)]
        let claude = [ProviderModelExposure(provider: "claude", model: nil, exposedInvocations: 10)]
        #expect(contextRevisionComparisonBlocker(
            earlierInvocations: 10,
            earlierCompleteCaptures: 9,
            laterInvocations: 10,
            laterCompleteCaptures: 9,
            earlierProviderModels: codex,
            laterProviderModels: claude,
            earlierFirstSeen: 100,
            earlierLastSeen: 200,
            laterFirstSeen: 300,
            laterLastSeen: 400
        )?.contains("provider/model mix") == true)
        #expect(contextRevisionComparisonBlocker(
            earlierInvocations: 10,
            earlierCompleteCaptures: 9,
            laterInvocations: 10,
            laterCompleteCaptures: 9,
            earlierProviderModels: codex,
            laterProviderModels: codex,
            earlierFirstSeen: 100,
            earlierLastSeen: 200,
            laterFirstSeen: 300,
            laterLastSeen: 550
        )?.contains("observation spans") == true)
        #expect(contextRevisionComparisonBlocker(
            earlierInvocations: 10,
            earlierCompleteCaptures: 9,
            laterInvocations: 10,
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
        let query = InvocationSetQuery(
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
            issue: "INT-42",
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
        #expect(decoded.wave == "intelligence")
        #expect(decoded.repoPath == "/src/loopflow")
    }

    @Test("Refinement uses the Wave's sole Project or its remembered Task destination")
    func refinementResolvesOneOwningProject() throws {
        let context = WaveProject(id: "context", title: "Context")
        let architecture = WaveProject(id: "architecture", title: "Architecture")

        #expect(try contextRefinementProject([context], projectId: nil).id == "context")
        #expect(try contextRefinementProject(
            [context, architecture],
            projectId: "architecture"
        ).id == "architecture")
        #expect(throws: ContextRefinementError.self) {
            _ = try contextRefinementProject([context, architecture], projectId: nil)
        }
        #expect(throws: ContextRefinementError.self) {
            _ = try contextRefinementProject([context], projectId: "removed")
        }
    }

    @Test("Task refinement carries main's source identity into the worker directive")
    func refinementDirectiveNamesTheSourceAndRevision() throws {
        let path = try contextRefinementSourcePath(
            sourcePath: "/src/loopflow/rust/loopflow/src/engine/builtins/LOOPFLOW.md",
            repoPath: "/src/loopflow"
        )
        #expect(path == "rust/loopflow/src/engine/builtins/LOOPFLOW.md")
        #expect(contextRefinementTaskTitle(
            label: "LOOPFLOW.md",
            contentSha256: "5e41e69b01234567"
        ) == "Refine LOOPFLOW.md 5e41e69b")
        let fixture = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/context_lab_snapshot.json")
        let snapshot = try JSONDecoder().decode(
            ContextLabSnapshot.self,
            from: Data(contentsOf: fixture)
        )
        let evidence = try #require(snapshot.evidence.first)
        let directive = try contextRefinementDirective(
            label: "LOOPFLOW.md",
            sourcePath: path,
            sourceSha256: "raw-source-hash",
            seed: RefinementSeed(
                query: snapshot.query,
                selectedNodeId: evidence.nodeId,
                sourcePath: path,
                startingContentSha256: evidence.contentSha256,
                measurements: evidence.measurements,
                evidence: evidence.representatives.map(\.address)
            )
        )
        #expect(directive.hasPrefix("Refine text for LOOPFLOW.md."))
        #expect(directive.contains("`rust/loopflow/src/engine/builtins/LOOPFLOW.md`"))
        #expect(directive.contains("raw-source-hash"))
        #expect(throws: ContextRefinementError.self) {
            _ = try contextRefinementSourcePath(
                sourcePath: "/tmp/unrelated.md",
                repoPath: "/src/loopflow"
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

import Foundation
import Testing

@testable import Loopflow

@Suite("Work Census")
struct WorkCensusTests {
    @Test("Only User-attention Launch rows are openable")
    func projectionAssignsLaunchIdentityOnlyToUserAttention() throws {
        let roadmap = try minimalRoadmap()
        let userLaunch = launch(id: "launch-user", attentionKind: "user")
        let parentLaunch = launch(id: "launch-parent", attentionKind: "parent")

        let census = WorkCensus(
            roadmap: roadmap,
            runs: [],
            launches: [userLaunch, parentLaunch]
        )
        let wave = try #require(census.groups.first { $0.id == "wave-1" })
        let userRow = try #require(wave.rows.first { $0.id == "launch:launch-user" })
        let parentRow = try #require(wave.rows.first { $0.id == "launch:launch-parent" })

        #expect(userRow.kind == .launch)
        #expect(userRow.launchId == userLaunch.id)
        #expect(userRow.isOpenable)

        #expect(parentRow.kind == .launch)
        #expect(parentRow.launchId == nil)
        #expect(!parentRow.isOpenable)

        let workRows = wave.rows.filter { $0.kind != .launch }
        #expect(workRows.map(\.kind) == [.project, .task])
        #expect(workRows.allSatisfy { $0.launchId == nil && !$0.isOpenable })
    }

    private func launch(id: String, attentionKind: String) -> LaunchSurfaceRecord {
        LaunchSurfaceRecord(
            launch: LaunchRecord(
                id: id,
                runId: "run-\(id)",
                homeId: "home-1",
                route: LaunchRouteRecord(provider: "opaque", model: nil, accountId: nil),
                cwd: "/src/loopflow.task",
                surface: "terminal",
                state: .live,
                containment: .processGroup(id: 1),
                opaqueBasis: nil,
                resumeToken: nil,
                startedAt: "2026-07-17T12:00:00Z",
                endedAt: nil
            ),
            work: WorkReference(
                kind: .task,
                id: "ts_now00000000000000000000000000000"
            ),
            waveId: "wave-1",
            homeRoute: "local",
            attention: LaunchAttentionRecord(
                kind: attentionKind,
                work: attentionKind == "parent"
                    ? WorkReference(
                        kind: .project,
                        id: "ps_11111111111111111111111111111111"
                    )
                    : nil
            ),
            attentionAt: "2026-07-17T12:01:00Z",
            handback: nil,
            attachArgv: nil
        )
    }

    private func minimalRoadmap(sourceFile: String = #filePath) throws -> RoadmapSnapshot {
        let fixture = URL(fileURLWithPath: sourceFile)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")
        let decoded = try JSONDecoder().decode(
            RoadmapSnapshot.self,
            from: Data(contentsOf: fixture)
        )
        let sourceWave = try #require(decoded.waves.first)
        let sourceProject = try #require(sourceWave.projects.items.first)
        let sourceTask = try #require(sourceProject.tasks.first)
        let project = RoadmapProject(
            project: sourceProject.project,
            runtime: sourceProject.runtime,
            nextMove: sourceProject.nextMove,
            section: sourceProject.section,
            tasks: [sourceTask]
        )
        return RoadmapSnapshot(
            generatedAt: decoded.generatedAt,
            waves: [
                WaveRoadmap(
                    wave: sourceWave.wave,
                    projects: .available(items: [project], truncated: false)
                )
            ]
        )
    }
}

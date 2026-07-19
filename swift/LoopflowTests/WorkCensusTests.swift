import Foundation
import Testing

@testable import Loopflow

@Suite("Work Census")
struct WorkCensusTests {
    @Test("Invocation rows are openable only with an attach command")
    func projectionAssignsInvocationIdentityOnlyToAttachableInvocations() throws {
        let roadmap = try minimalRoadmap()
        let userInvocation = invocation(id: "invocation-user", attachable: true)
        let parentInvocation = invocation(id: "invocation-parent", attachable: false)

        let census = WorkCensus(
            roadmap: roadmap,
            runs: [],
            invocations: [userInvocation, parentInvocation]
        )
        let wave = try #require(census.groups.first { $0.id == "wave-1" })
        let userRow = try #require(wave.rows.first { $0.id == "invocation:invocation-user" })
        let parentRow = try #require(wave.rows.first { $0.id == "invocation:invocation-parent" })

        #expect(userRow.kind == .invocation)
        #expect(userRow.invocationId == userInvocation.id)
        #expect(userRow.isOpenable)

        #expect(parentRow.kind == .invocation)
        #expect(parentRow.invocationId == nil)
        #expect(!parentRow.isOpenable)

        let workRows = wave.rows.filter { $0.kind != .invocation }
        #expect(workRows.map(\.kind) == [.project, .task])
        #expect(workRows.allSatisfy { $0.invocationId == nil && !$0.isOpenable })
    }

    private func invocation(id: String, attachable: Bool) -> InvocationSurfaceRecord {
        let work = WorkReference(
            kind: .task,
            id: "ts_now00000000000000000000000000000"
        )
        return InvocationSurfaceRecord(
            invocation: AgentInvocationRecord(
                id: id,
                supervisingRunId: "run-\(id)",
                route: InvocationRouteRecord(provider: "opaque", model: nil, accountId: nil),
                surface: "terminal",
                resumeToken: nil,
                startedAt: "2026-07-17T12:00:00Z",
                endedAt: nil
            ),
            run: RunRecord(
                id: "run-\(id)",
                work: work,
                epochId: "epoch-1",
                homeId: "home-1",
                state: .active,
                trigger: .user,
                retryOf: nil,
                containment: .processGroup(id: 1),
                cwd: "/src/loopflow.task",
                createdAt: "2026-07-17T11:59:59Z",
                startedAt: "2026-07-17T12:00:00Z",
                endedAt: nil
            ),
            work: work,
            waveId: "wave-1",
            homeRoute: "local",
            handback: nil,
            attachArgv: attachable ? ["tmux", "attach-session", "-t", "lf-task"] : nil
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

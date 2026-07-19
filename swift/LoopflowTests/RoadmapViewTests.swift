#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Roadmap controls")
struct RoadmapViewTests {
    @Test("Task actions come from the shared legal-action model")
    func taskActionsFollowTheActionModel() throws {
        let snapshot = try loadRoadmapFixture()
        let project = try #require(snapshot.waves.first?.projects.items.first)
        let tasks = project.tasks

        // A live body with no lifecycle move waiting on it falls back to Attach.
        #expect(roadmapTaskAction(tasks[0]) == .attach)
        #expect(roadmapTaskCanInterrupt(tasks[0]))

        // Waiting on an open PR advertises Review, not Resume: the server
        // recommends review, and the app must not re-derive Resume from status.
        #expect(tasks[1].attention.actions.recommended == .openPr)
        #expect(roadmapTaskAction(tasks[1]) == .openPr)
        #expect(!roadmapTaskCanInterrupt(tasks[1]))

        // No Session yet, and a terminal Task offers nothing.
        #expect(roadmapTaskAction(tasks[2]) == .run)
        #expect(roadmapTaskAction(tasks[3]) == nil)
    }

    @Test("The recommended action explains itself")
    func recommendedActionCarriesItsReason() throws {
        let snapshot = try loadRoadmapFixture()
        let project = try #require(snapshot.waves.first?.projects.items.first)
        let reviewing = project.tasks[1].attention.actions

        #expect(reviewing.recommended == .openPr)
        #expect(reviewing.reason == "checks passed; awaiting review")
    }

    private func loadRoadmapFixture(sourceFile: String = #filePath) throws -> RoadmapSnapshot {
        let fixture = URL(fileURLWithPath: sourceFile)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")
        return try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(contentsOf: fixture))
    }
}
#endif

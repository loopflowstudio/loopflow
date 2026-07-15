#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Roadmap controls")
struct RoadmapViewTests {
    @Test("Task actions come from the shared roadmap runtime evidence")
    func taskActionsFollowRuntimeEvidence() throws {
        let snapshot = try loadRoadmapFixture()
        let project = try #require(snapshot.waves.first?.projects.items.first)
        let tasks = project.tasks

        #expect(roadmapTaskAction(tasks[0]) == .attach)
        #expect(roadmapTaskCanInterrupt(tasks[0]))
        #expect(roadmapTaskAction(tasks[1]) == .resume)
        #expect(!roadmapTaskCanInterrupt(tasks[1]))
        #expect(roadmapTaskAction(tasks[2]) == .run)
        #expect(roadmapTaskAction(tasks[3]) == nil)
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

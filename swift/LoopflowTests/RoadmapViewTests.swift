#if os(macOS)
import Foundation
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Roadmap controls")
struct RoadmapViewTests {
    @Test("Task actions come from the shared legal-action model")
    func taskActionsFollowTheActionModel() throws {
        let snapshot = try loadRoadmapFixture()
        let wave = try #require(snapshot.waves.first)
        let tasks = wave.tasks.items

        // Continuing started work is a fresh Start, never an inferred attach
        // or interrupt action.
        #expect(tasks[0].actions.recommended == .run)
        #expect(roadmapTaskAction(tasks[0]) == .run)

        // An explicit User merge request advertises Open PR: the server
        // recommends the exact head, and the app does not re-derive it.
        #expect(tasks[1].actions.recommended == .openPr)
        #expect(roadmapTaskAction(tasks[1]) == .openPr)

        // No Task Work yet, and a terminal Task offers nothing.
        #expect(roadmapTaskAction(tasks[2]) == .run)
        #expect(roadmapTaskAction(tasks[3]) == nil)
    }

    @Test("The recommended action explains itself")
    func recommendedActionCarriesItsReason() throws {
        let snapshot = try loadRoadmapFixture()
        let wave = try #require(snapshot.waves.first)
        let openingPR = wave.tasks.items[1].actions

        #expect(openingPR.recommended == .openPr)
        #expect(openingPR.reason == "merge head abc1234 on GitHub")
    }

    @Test @MainActor func portfolioTaskClickOpensItsRepositoryDestination() throws {
        let snapshot = try loadRoadmapFixture()
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("No read needed") })
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        var opened: (String, String)?
        let view = RoadmapView(model: model, onOpenWave: { _ in }, onOpenTask: { opened = ($0, $1) })
        let section = try view.inspect().find(NowSectionView.self).actualView()
        let row = try #require(section.section.rows.first)
        try view.inspect().find(viewWithAccessibilityIdentifier: "loopflow-task-\(row.task.id)").callOnTapGesture()
        #expect(opened?.0 == row.wave!.repo)
        #expect(opened?.1 == row.task.id)
        #expect(model.selection == nil)
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

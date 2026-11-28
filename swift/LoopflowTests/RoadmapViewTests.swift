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
        // Its Graph has run, so the row says what its page says.
        #expect(RoadmapTaskAction.run.label(tasks[0]) == "Run")

        // An explicit User merge request advertises Open PR: the server
        // recommends the exact head, and the app does not re-derive it.
        #expect(tasks[1].actions.recommended == .openPr)
        #expect(roadmapTaskAction(tasks[1]) == .openPr)

        // No Task Work yet, and a terminal Task offers nothing.
        #expect(roadmapTaskAction(tasks[2]) == .run)
        #expect(RoadmapTaskAction.run.label(tasks[2]) == "Start")
        #expect(roadmapTaskAction(tasks[3]) == nil)

        let parked = try roadmapTask(condition: "waiting", reason: "At breakpoint after-explore.",
                                     lifecycle: "active", recommended: "run",
                                     selectionStates: ["open", "active", "working", "breakpoint"])
        #expect(roadmapTaskAction(parked) == .run)
        #expect(RoadmapTaskAction.run.label(parked) == "Continue")
    }

    @Test @MainActor func continueFromRoadmapSendsTheDisplayedArrival() async throws {
        let arrival = "11111111-1111-4111-8111-111111111111"
        let task = try roadmapTask(condition: "waiting", reason: "At breakpoint after-explore.",
                                   lifecycle: "active", recommended: "run", selectionStates: ["breakpoint"],
                                   breakpointArrival: arrival)
        let expected = ["task", "run", task.task.identifier, "--at-arrival", arrival, "--json"]
        let machine = HeldMachine()
        let query = RegistryQuery(change: { write, requestID in
            #expect(write.argv == expected)
            return try await machine.send(write, requestID)
        }) { _, _ in throw RegistryQueryError("No read needed") }
        let model = WorkModel(query: query)
        model.perform(.run, task: task)
        await machine.answer(nil)
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
        try view.inspect().find(viewWithAccessibilityIdentifier: "loopflow-task-\(row.task.id)")
            .find(ViewType.Button.self, where: { try $0.accessibilityIdentifier() == "task-row-open-\(row.task.id)" }).tap()
        // The row says why the Task is on the Now list.
        #expect(try view.inspect().find(text: row.task.condition.reason).string() == row.task.condition.reason)
        #expect(opened?.0 == row.wave!.repo)
        #expect(opened?.1 == row.task.id)
        #expect(model.selection == nil)
    }

    @Test @MainActor func aWaveOnTheRoadmapIsASectionUnderItsNameWithNoFrame() throws {
        let snapshot = try loadRoadmapFixture()
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("No read needed") })
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [])
        let wave = try #require(model.visibleRoadmaps.first)
        let section = try RoadmapWaveSection(roadmap: wave, onOpenWave: {}, onOpenTask: { _ in },
                                             onTaskAction: { _, _ in }, onOpenWorktree: { _ in })
            .inspect().find(viewWithAccessibilityIdentifier: "loopflow-wave-\(wave.wave.id)")
        #expect(try section.find(text: wave.wave.name).string() == wave.wave.name)
        #expect(throws: (any Error).self) { try section.overlay() }
        #expect(throws: (any Error).self) { try section.background() }
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

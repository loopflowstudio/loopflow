#if os(macOS)
import Foundation
import XCTest
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

extension ClientWorkQueryTests {
    func testDesktopCompleteButtonPublishesBothWindowsAndOnlyNavigatesItsOriginBeforeDisk() async throws {
        let fixture = try WorkTestFixture.load()
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        var wire = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(workingPlan(["issue-now", "issue-review"]))) as? [String: Any])
        wire["repo_path"] = "/src/loopflow"
        var contents = try XCTUnwrap(wire["contents"] as? [[String: Any]])
        contents[0]["identifier"] = "W2-144"
        contents[1]["identifier"] = "W2-131"
        wire["contents"] = contents
        let base = try JSONDecoder().decode(WorkViewPlan.self, from: JSONSerialization.data(withJSONObject: wire))
        let scope = WorkScope(repo: "/src/loopflow", headless: false, task: nil, activity: nil, taskListWave: nil)
        let query = try XCTUnwrap(WorkPart.taskList.read(for: scope))
        let seed = store(directory)
        _ = try await seed.receivePlans([base], stamp: stamp("list", revision: 6), for: query)
        let prepared = try await seed.presentation(machine: "machine", queries: [key(query): query])
        let hold = HeldIntentDisk()
        let disk = ClientWorkStore(directory: directory, prepareDirectory: { _ in hold.enter() }, machine: { "machine" },
            read: { _ in "null" }, observe: { _ in ServerWatch(updates: AsyncThrowingStream { _ in }, checkpoint: { _ in }, cancel: {}) },
            send: { _ in XCTFail("The gesture must not send") })
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { hold.release(); owner.shutdown() }
        _ = try owner.open(query) { _ in }
        owner.apply(prepared)
        var entered = hold.entered.makeAsyncIterator()
        _ = await entered.next()
        func window() -> WorkModel {
            let model = WorkModel(query: fixture.query, repoPath: "/src/loopflow")
            model.useInteractionForTesting(owner)
            model.confirmMachine("machine")
            model.applyFixture(roadmap: .available(fixture.roadmap), waves: .available(fixture.waves),
                workActivity: .available(fixture.workActivity), repos: [])
            model.select(.task(id: "issue-now"))
            model.syncWorkScope()
            return model
        }
        let first = window()
        let second = window()
        let task = try XCTUnwrap(first.task(id: "issue-now")?.task)
        let button = TaskWorkflowHeader(model: first, task: task, work: .loading)
        try button.inspect().find(viewWithAccessibilityIdentifier: "task-complete").button().tap()
        XCTAssertEqual(first.sidebarTaskIDs, ["issue-review"])
        XCTAssertEqual(second.sidebarTaskIDs, ["issue-review"])
        XCTAssertEqual(first.taskListReading.value?.taskCount, 1)
        XCTAssertEqual(first.selection, .task(id: "issue-review"))
        XCTAssertEqual(second.selection, .task(id: "issue-now"))
        hold.release()
        await owner.persistPending()
        XCTAssertEqual(first.sidebarTaskIDs, ["issue-review"])
    }
}
#endif

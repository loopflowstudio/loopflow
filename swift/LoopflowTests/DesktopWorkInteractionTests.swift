#if os(macOS)
import Foundation
import Observation
import XCTest
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

extension ClientWorkQueryTests {
    func testDesktopCompleteButtonPublishesBothWindowsAndOnlyNavigatesItsOriginBeforeDisk() async throws {
        try await leaveBothWindows(abandon: false)
    }

    func testDesktopAbandonPublishesBothWindowsAndOnlyNavigatesItsOriginBeforeDisk() async throws {
        try await leaveBothWindows(abandon: true)
    }

    func testDesktopMoveRemainsAvailableOfflineAndKeepsBothSelections() async throws {
        try await leaveBothWindows(abandon: false, move: true)
    }

    private func leaveBothWindows(abandon: Bool, move: Bool = false) async throws {
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
        _ = owner.open(query) { _ in }
        owner.apply(prepared)
        var entered = hold.entered.makeAsyncIterator()
        _ = await entered.next()
        func window() -> WorkModel {
            let model = WorkModel(query: fixture.query, repoPath: "/src/loopflow")
            model.useInteractionForTesting(owner)
            model.confirmMachine("machine")
            model.applyFixture(roadmap: .unavailable(lastGood: fixture.roadmap, reason: "Machine offline"), waves: .available(fixture.waves),
                workActivity: .available(fixture.workActivity), repos: [])
            model.select(.task(id: "issue-now"))
            model.syncWorkScope()
            return model
        }
        let first = window()
        let second = window()
        let task = try XCTUnwrap(first.task(id: "issue-now")?.task)
        let controls = TaskWorkflowHeader(model: first, task: task,
            work: .unavailable(lastGood: nil, reason: "Machine offline"))
        if move {
            let menu = TaskActionItems(model: first, task: task, wave: nil)
            XCTAssertFalse(try menu.inspect().find(viewWithAccessibilityIdentifier: "task-move-wave").isDisabled())
            try menu.inspect().find(button: "No Wave").tap()
            XCTAssertEqual(first.selection, .task(id: "issue-now"))
            XCTAssertEqual(second.selection, .task(id: "issue-now"))
            hold.release()
            await owner.persistPending()
            let records = try await disk.pending()
            XCTAssertEqual(records.count, 1)
            XCTAssertEqual(records.first?.destination, "machine")
            return
        }
        if abandon {
            XCTAssertFalse(try controls.inspect().find(viewWithAccessibilityIdentifier: "task-abandon").button().isDisabled())
            await first.abandonTask(task)
        } else {
            let menu = TaskActionItems(model: first, task: task, wave: nil)
            XCTAssertFalse(try menu.inspect().find(button: "Complete Task").isDisabled())
            let button = try controls.inspect().find(viewWithAccessibilityIdentifier: "task-complete").button()
            XCTAssertFalse(try button.isDisabled())
            try button.tap()
        }
        XCTAssertTrue(first.pendingOperations.isEmpty)
        XCTAssertTrue(second.pendingOperations.isEmpty)
        XCTAssertEqual(first.sidebarTaskIDs, ["issue-review"])
        XCTAssertEqual(second.sidebarTaskIDs, ["issue-review"])
        XCTAssertEqual(first.taskListReading.value?.taskCount, 1)
        XCTAssertEqual(first.selection, .task(id: "issue-review"))
        XCTAssertEqual(second.selection, .task(id: "issue-now"))
        hold.release()
        await owner.persistPending()
        XCTAssertEqual(first.sidebarTaskIDs, ["issue-review"])
    }
    func testCachedQueryNavigationAndOriginOrderDuringHeldCompleteSave() async throws {
        let fixture = try WorkTestFixture.load()
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        var wire = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(workingPlan(["issue-now", "issue-review", "third"]))) as? [String: Any])
        wire["repo_path"] = "/src/loopflow"
        var contents = try XCTUnwrap(wire["contents"] as? [[String: Any]])
        contents[0]["identifier"] = "W2-144"
        wire["contents"] = contents
        let base = try JSONDecoder().decode(WorkViewPlan.self, from: JSONSerialization.data(withJSONObject: wire))
        let a = ServedRead.desktopViewGet(directory: "/src/loopflow", expression: "state:working")
        let b = ServedRead.desktopViewGet(directory: "/src/loopflow", expression: "state:available")
        let seed = store(directory)
        _ = try await seed.receivePlans([base], stamp: stamp("a", revision: 6), for: a)
        _ = try await seed.receivePlans([base], stamp: stamp("b", revision: 6), for: b)
        let prepared = try await seed.presentation(machine: "machine", queries: [key(a): a, key(b): b])
        let hold = HeldIntentDisk()
        let disk = ClientWorkStore(directory: directory, prepareDirectory: { _ in hold.enter() }, machine: { "machine" },
            read: { _ in "null" }, observe: { _ in ServerWatch(updates: AsyncThrowingStream { _ in }, checkpoint: { _ in }, cancel: {}) }, send: { _ in })
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { hold.release(); owner.shutdown() }
        _ = owner.open(a) { _ in }; _ = owner.open(b) { _ in }
        owner.apply(prepared)
        var entered = hold.entered.makeAsyncIterator()
        _ = await entered.next()
        let query = RegistryQuery(subscribeRead: { _, _ in
            let (lines, continuation) = AsyncThrowingStream<ReceivedReading, any Error>.makeStream()
            return LineSubscription(lines: lines, send: { _ in }, cancel: { continuation.finish() })
        }) { _, _ in throw RegistryQueryError("No one-shot query publication") }
        let model = WorkModel(query: query, repoPath: "/src/loopflow")
        defer { model.select(nil) }
        model.useInteractionForTesting(owner)
        model.confirmMachine("machine")
        model.applyFixture(roadmap: .available(fixture.roadmap), waves: .available(fixture.waves), workActivity: .loading, repos: [])
        model.applySelectionPlans([base], reason: "")
        model.select(.task(id: "issue-now"))
        let task = try XCTUnwrap(model.task(id: "issue-now")?.task)
        // This origin's next occurrence is third; the sidebar would choose issue-review.
        let order = [base.contents[0].work, base.contents[2].work, base.contents[1].work, base.contents[0].work]
        model.completeTask(task, visible: order)
        XCTAssertEqual(model.selection, .task(id: "third"))
        await model.openView(expression: "state:working")
        let destination = model.navigation.destination
        XCTAssertEqual(model.selectedViewPlan?.taskCount, 2)
        await model.openView(expression: "state:available")
        XCTAssertEqual(model.selectedViewPlan?.taskCount, 2)
        model.goBack()
        XCTAssertEqual(model.navigation.destination, destination)
        XCTAssertEqual(model.selectedViewPlan?.projection.visible.map(\.work.id), ["issue-review", "third"])
        owner.apply(prepared)
        XCTAssertEqual(model.navigation.destination, destination)
        XCTAssertEqual(model.selectedViewPlan?.taskCount, 2)
        model.select(.task(id: "issue-review"))
        hold.release()
        await owner.persistPending()
        try await owner.receive([base], stamp: stamp("b", revision: 6), for: b)
        try await owner.receive([base], stamp: stamp("a", revision: 6), for: a)
        XCTAssertEqual(model.selection, .task(id: "issue-review"))
        model.goBack()
        XCTAssertEqual(model.navigation.destination, destination)
        XCTAssertEqual(model.selectedViewPlan?.taskCount, 2)
    }

    func testStampedSavedQueryRefreshAfterUnrelatedWriteKeepsPendingProjectionAndDestination() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let read = ServedRead.desktopViewGet(directory: "/repo", expression: "query:picked")
        let disk = store(directory)
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { owner.shutdown() }
        _ = owner.open(read) { _ in }
        try await owner.receive([workingPlan(["a", "b"])], stamp: stamp("query", revision: 6), for: read)
        _ = try complete(directory)
        await owner.refreshFromStore()
        let dashboard = String(decoding: try loadFixtureData("dashboard.json"), as: UTF8.self)
        let (connections, connecting) = AsyncStream<AsyncThrowingStream<ReceivedReading, any Error>.Continuation>.makeStream()
        var connectionsIterator = connections.makeAsyncIterator()
        let query = RegistryQuery(write: { _ in dashboard }, subscribeRead: { _, _ in
            let (lines, continuation) = AsyncThrowingStream<ReceivedReading, any Error>.makeStream()
            connecting.yield(continuation)
            return LineSubscription(lines: lines, send: { _ in }, cancel: { continuation.finish() })
        }) { _, _ in throw RegistryQueryError("No unstamped refresh") }
        let model = WorkModel(query: query, repoPath: "/repo")
        let other = WorkModel(query: query, repoPath: "/repo")
        defer { model.select(nil); other.select(nil); connecting.finish() }
        model.useInteractionForTesting(owner)
        model.confirmMachine("machine")
        model.selectDashboard("picked")
        let destination = model.navigation.destination
        XCTAssertEqual(model.selectedViewPlan?.taskCount, 1)
        let firstConnection = await connectionsIterator.next()
        let first = try XCTUnwrap(firstConnection)
        other.useInteractionForTesting(owner)
        other.confirmMachine("machine")
        other.selectDashboard("picked")
        XCTAssertEqual(other.selectedViewPlan?.taskCount, 1)
        let otherConnection = await connectionsIterator.next()
        XCTAssertNotNil(otherConnection)
        let content = try XCTUnwrap(model.selectedViewPlan?.contents.last)
        await model.setDashboardTile("unrelated", member: content.work, hidden: true)
        XCTAssertEqual(model.selectedViewPlan?.taskCount, 1)
        XCTAssertEqual(model.navigation.destination, destination)
        let failed = expectation(description: "Refresh failure is published")
        withObservationTracking { _ = model.dashboardError } onChange: { failed.fulfill() }
        first.finish(throwing: RegistryQueryError("Refresh unavailable"))
        await fulfillment(of: [failed], timeout: 10)
        XCTAssertEqual(model.selectedViewPlan?.taskCount, 1)
        XCTAssertEqual(model.navigation.destination, destination)
        XCTAssertEqual(model.dashboardError, "Refresh unavailable")
        other.select(nil)
        await owner.refreshFromStore()
        XCTAssertEqual(model.selectedViewPlan?.taskCount, 1, "One failed window keeps usable content when its sibling closes")
        model.retrySelectedQuery()
        XCTAssertEqual(model.selectedViewPlan?.taskCount, 1)
        let secondConnection = await connectionsIterator.next()
        let second = try XCTUnwrap(secondConnection)
        let refreshed = expectation(description: "Stamped replacement is published")
        withObservationTracking { _ = model.queryPlans } onChange: { refreshed.fulfill() }
        let plan = try workingPlan(["b"])
        let data = try JSONSerialization.data(withJSONObject: ["answers": NSNull(), "unavailable": NSNull(),
            "body": JSONSerialization.jsonObject(with: JSONEncoder().encode(plan)),
            "stamp": JSONSerialization.jsonObject(with: JSONEncoder().encode(stamp("query", revision: 7)))])
        second.yield(ReceivedReading(data))
        await fulfillment(of: [refreshed], timeout: 10)
        XCTAssertEqual(model.selectedViewPlan?.taskCount, 1)
        XCTAssertEqual(model.navigation.destination, destination)
        model.select(nil)
        owner.shutdown()
        let restarted = WorkInteraction(machine: "machine", store: store(directory))
        defer { restarted.shutdown() }
        model.useInteractionForTesting(restarted)
        model.selectDashboard("picked")
        XCTAssertEqual(model.navigation.destination, destination)
        await restarted.refreshFromStore()
        XCTAssertEqual(model.selectedViewPlan?.taskCount, 1)
    }

    func testDelayedDashboardSaveCannotSelectAfterLeavingAndReturningToItsExpression() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let dashboard = String(decoding: try loadFixtureData("dashboard.json"), as: UTF8.self)
        let (entered, entering) = AsyncStream<Void>.makeStream()
        let (release, releasing) = AsyncStream<Void>.makeStream()
        defer { entering.finish(); releasing.finish() }
        let query = RegistryQuery(write: { _ in
            entering.yield(())
            var released = release.makeAsyncIterator()
            _ = await released.next()
            return dashboard
        }, subscribeRead: { _, _ in
            let (lines, continuation) = AsyncThrowingStream<ReceivedReading, any Error>.makeStream()
            return LineSubscription(lines: lines, send: { _ in }, cancel: { continuation.finish() })
        }) { _, _ in throw RegistryQueryError("No unstamped read") }
        let owner = WorkInteraction(machine: "machine", store: store(directory))
        defer { owner.shutdown() }
        let model = WorkModel(query: query, repoPath: "/repo")
        model.useInteractionForTesting(owner)
        model.confirmMachine("machine")
        defer { model.select(nil) }
        await model.openView(expression: "state:working")
        let original = model.navigation.destination
        let save = Task { await model.saveDashboard(id: "picked", expression: "state:working", name: "Picked", pinned: false, revision: 3) }
        var entries = entered.makeAsyncIterator()
        _ = await entries.next()
        await model.openView(expression: "state:available")
        model.goBack()
        XCTAssertEqual(model.navigation.destination, original)
        releasing.yield(())
        await save.value
        XCTAssertEqual(model.navigation.destination, original)
        XCTAssertNil(model.navigation.dashboardID)
    }

    func testColdQuerySelectsBeforeItsReadAndKeepsItsFailureInThatDestination() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let owner = WorkInteraction(machine: "machine", store: store(directory))
        defer { owner.shutdown() }
        let query = RegistryQuery(subscribeRead: { _, _ in
            LineSubscription(lines: AsyncThrowingStream { $0.finish(throwing: RegistryQueryError("Query unavailable")) },
                send: { _ in }, cancel: {})
        }) { _, _ in throw RegistryQueryError("No raw answer") }
        let model = WorkModel(query: query, repoPath: "/repo")
        model.useInteractionForTesting(owner)
        model.confirmMachine("machine")
        let registry = SessionsWorkspaceRegistry(localMachineId: "machine")
        model.bindWorkspaces(registry, repository: "repo")
        defer { model.select(nil) }
        let failed = expectation(description: "Cold query failure")
        withObservationTracking { _ = model.dashboardError } onChange: { failed.fulfill() }
        await model.openView(expression: "state:working")
        let destination = model.navigation.destination
        XCTAssertEqual(destination, .view(.init(machine: "machine", repository: "/repo", expression: "state:working")))
        XCTAssertTrue(model.queryPlans.isLoading)
        XCTAssertNil(model.selectedViewPlan)
        await fulfillment(of: [failed], timeout: 10)
        XCTAssertEqual(model.navigation.destination, destination)
        let inspection = model.inspectDesktop(repository: "repo", window: UUID(), workspaces: registry)
        XCTAssertEqual(inspection.opening?.status, .failed)
        XCTAssertEqual(model.queryPlans.errorMessage, "Query unavailable")
    }

}
#endif

import Foundation
import XCTest
import ViewInspector
@testable import Loopflow
@testable import LoopflowiOS

extension ClientWorkQueryTests {
    func testPhoneIncompleteDestinationKeepsKnownRowsAndMarksItsCountUnavailable() throws {
        let original = try workingPlan(["a", "b"])
        let membership = WorkMembershipPlan(domains: original.membership.domains, effects: original.membership.effects,
            root: original.membership.root, facts: original.membership.facts,
            unavailable: ["Destination membership awaits confirmation"])
        let projected = ProjectedWorkPlan(original, membership: membership)
        let repositories = try RemoteTaskObservation.decodePlans([projected]).get()
        XCTAssertEqual(repositories.first?.tasks.map(\.work.id), ["a", "b"])
        XCTAssertFalse(try XCTUnwrap(repositories.first).unavailable.isEmpty)
        XCTAssertNil(projected.taskCount)
    }

    func testPhoneCompleteButtonPublishesBothFeedsAndReplacesOnlyTheOriginRouteBeforeDisk() async throws {
        try await leaveBothFeeds(abandon: false)
    }

    func testPhoneAbandonPublishesBothFeedsAndReplacesOnlyTheOriginRouteBeforeDisk() async throws {
        try await leaveBothFeeds(abandon: true)
    }

    func testUnreachableMachineShowsWaitingForColdAndSavedFeedsAndRefreshReturns() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let connection = MachineConnection { throw ServerConnectionUnavailable(RegistryQueryError("Offline")) }
        _ = try? await connection.link()
        let disk = ClientWorkStore(directory: directory, connection: connection, machine: { "machine" },
            read: { _ in "null" }, observe: { _ in
                ServerWatch(updates: AsyncThrowingStream { _ in }, checkpoint: { _ in }, cancel: {})
            }, send: { _ in XCTFail("No send while disconnected") })
        addTeardownBlock { await disk.close() }
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { stop(owner) }
        let client = RemoteMobileClient(savedMachine: SavedMachine(endpoint: URL(string: "https://machine.invalid")!, machineID: "machine"))
        client.journal = disk
        client.interaction = owner
        let feed = TaskFeed(client: client)
        feed.mount()
        defer { feed.stop() }
        await owner.refreshFromStore()
        XCTAssertTrue(feed.waitingForConnection)
        XCTAssertFalse(feed.loaded)
        XCTAssertNil(feed.error)
        let query = ServedRead.viewSelect(expression: TaskList.started.expression(wave: nil))
        let base = try workingPlan(["a", "b", "c", "d"])
        _ = try await disk.receivePlans([base], stamp: stamp("list", revision: 6), for: query)
        for issue in ["a", "b", "c"] {
            try await disk.save(SubmittedOperation(requestID: issue, destination: "machine",
                payload: JSONEncoder().encode(ServedWrite.taskComplete(directory: "/repo", issue: issue)),
                version: ServerProtocol.revision, submittedAt: Date()))
        }
        await owner.refreshFromStore()
        XCTAssertTrue(feed.loaded)
        XCTAssertTrue(feed.waitingForConnection)
        XCTAssertEqual(feed.repositories.first?.tasks.map(\.work.id), ["d"])
        XCTAssertEqual(owner.listedOperations.map(\.title), ["Complete Task · a", "Complete Task · b", "Complete Task · c"])
        let observation = RemoteTaskObservation(query: query, interaction: owner, connection: connection)
        defer {
            let closed = observation.close()
            addTeardownBlock { await closed.value }
        }
        try await observation.refresh()
        XCTAssertTrue(connection.isWaiting, "Refresh observes the existing reconnect, without waiting for it")
    }

    func testPhoneMoveMenuKeepsBothRoutesBeforeDiskWhileDisconnected() async throws {
        try await leaveBothFeeds(abandon: false, move: true)
    }

    func testPhoneOwnershipActionsPublishBeforeDiskAndKeepBothRoutes() async throws {
        try await leaveBothFeeds(abandon: false, ownership: true)
    }

    func testPhoneReopenButtonPublishesBeforeDiskAndKeepsBothRoutes() async throws {
        try await leaveBothFeeds(abandon: false, reopening: true)
    }

    func testPhoneReopenCancelsSupportedPendingCompletionBeforeDone() async throws {
        try await leaveBothFeeds(abandon: false, reopening: true, pending: true)
    }

    func testExecutionRequestsPublishBeforeDiskWithoutPredictingRuntime() async throws {
        try await leaveBothFeeds(abandon: false, execution: true)
    }

    private func leaveBothFeeds(abandon: Bool, move: Bool = false, ownership: Bool = false, reopening: Bool = false, pending: Bool = false, execution: Bool = false) async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let query = ServedRead.viewSelect(expression: TaskList.started.expression(wave: nil))
        var base = try workingPlan(["a", "b"])
        if reopening {
            var wire = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(base)) as? [String: Any])
            var membership = base.membership
            membership.facts[0].disposition.value.completed = !pending
            membership.facts[0].disposition.value.completionRequest = pending ? 99 : nil
            wire["membership"] = try JSONSerialization.jsonObject(with: JSONEncoder().encode(membership))
            base = try JSONDecoder().decode(WorkViewPlan.self, from: JSONSerialization.data(withJSONObject: wire))
        }
        let seed = store(directory)
        _ = try await seed.receivePlans([base], stamp: stamp("list", revision: 6), for: query)
        let otherQuery = ServedRead.viewSelect(expression: TaskList.available.expression(wave: nil))
        _ = try await seed.receivePlans([base], stamp: stamp("other", revision: 6), for: otherQuery)
        let prepared = try await seed.presentation(machine: "machine", queries: [key(query): query, key(otherQuery): otherQuery])
        let hold = HeldIntentDisk()
        let disk = ClientWorkStore(directory: directory, prepareDirectory: { _ in hold.enter() }, machine: { "machine" },
            read: { _ in "null" }, observe: { _ in ServerWatch(updates: AsyncThrowingStream { _ in }, checkpoint: { _ in }, cancel: {}) },
            send: { _ in XCTFail("The gesture must not send") })
        addTeardownBlock { await disk.close() }
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { hold.release(); stop(owner) }
        let client = RemoteMobileClient(savedMachine: SavedMachine(endpoint: URL(string: "https://machine.invalid")!, machineID: "machine"))
        client.journal = disk
        client.interaction = owner
        let first = TaskFeed(client: client)
        let second = TaskFeed(client: client)
        first.mount(); second.mount()
        defer { first.stop(); second.stop() }
        _ = owner.open(otherQuery) { _ in }
        owner.apply(prepared)
        var entered = hold.entered.makeAsyncIterator()
        _ = await entered.next()
        var route: WorkWorkspaceKey? = base.contents[0].work
        let otherRoute = route
        if execution {
            let task = try XCTUnwrap(first.repositories.first?.tasks.first)
            let states = first.taskFacts[task.work]?.states
            let menu = TaskExecutionActions { first.execute(task, action: $0) }
            try menu.inspect().find(button: "Start").tap()
            XCTAssertEqual(owner.operations.last?.presentation().text, "Start requested…")
            try menu.inspect().find(button: "Auto-Ship").tap()
            XCTAssertEqual(owner.operations.last?.presentation().text, "Auto-Ship requested…")
            try menu.inspect().find(button: "Stop Auto-Ship").tap()
            XCTAssertEqual(owner.operations.last?.presentation().text, "Stop Auto-Ship requested…")
            XCTAssertEqual(first.taskFacts[task.work]?.states, states)
            XCTAssertEqual(second.taskFacts[task.work]?.states, states)
            XCTAssertEqual(route, otherRoute)
            hold.release()
            await owner.persistPending()
            let records = try await disk.pending()
            XCTAssertEqual(records.count, 3)
            XCTAssertTrue(records.allSatisfy { $0.resource == .task(task.work) && $0.destination == "machine" })
            XCTAssertEqual(try records.map { try JSONDecoder().decode(JSONValue.self, from: $0.payload)["kind"] },
                [.string("task.run"), .string("task.ship"), .string("task.hold")])
            return
        }
        if reopening {
            let task = base.contents[0]
            let menu = TaskOwnershipActions(completed: true, owned: first.taskFacts[task.work]?.owned) { first.edit(task, edit: $0) }
            try menu.inspect().find(button: "Reopen").tap()
            XCTAssertFalse(first.taskFacts[task.work]?.states.contains(.done) == true)
            XCTAssertFalse(second.taskFacts[task.work]?.states.contains(.done) == true)
            XCTAssertNil(first.taskFacts[task.work]?.disposition.value.completionRequest)
            XCTAssertEqual(route?.id, "a")
            XCTAssertEqual(otherRoute?.id, "a")
            hold.release()
            await owner.persistPending()
            let records = try await disk.pending()
            XCTAssertEqual(records.count, 1)
            return
        }
        if ownership {
            let task = try XCTUnwrap(first.repositories.first?.tasks.first)
            let menu = TaskOwnershipActions(completed: false, owned: first.taskFacts[task.work]?.owned) { first.edit(task, edit: $0) }
            try menu.inspect().find(button: "Unassign").tap()
            XCTAssertEqual(first.taskFacts[task.work]?.owned, false)
            XCTAssertEqual(second.taskFacts[task.work]?.owned, false)
            first.edit(task, edit: .claim)
            XCTAssertEqual(second.taskFacts[task.work]?.owned, true)
            first.edit(task, edit: .reopen)
            hold.release()
            await owner.persistPending()
            let records = try await disk.pending()
            XCTAssertEqual(records.count, 3)
            XCTAssertTrue(records.allSatisfy { $0.destination == "machine" })
            return
        }
        if move {
            let menu = TaskMoveMenu(waves: []) { first.move(base.contents[0], to: $0) }
            try menu.inspect().find(button: "No Wave").tap()
            XCTAssertEqual(route?.id, "a")
            XCTAssertEqual(otherRoute?.id, "a")
            hold.release()
            await owner.persistPending()
            let records = try await disk.pending()
            XCTAssertEqual(records.count, 1)
            XCTAssertEqual(records.first?.destination, "machine")
            first.stop(); second.stop()
            return
        }
        if abandon {
            first.abandon(base.contents[0], selected: route, visible: base.contents.map(\.work)) { route = $0 }
        } else {
        let button = TaskCompleteButton {
            first.complete(base.contents[0], selected: route, visible: base.contents.map(\.work)) { route = $0 }
        }
        try button.inspect().find(button: "Complete").tap()
        }
        XCTAssertTrue(owner.listedOperations.isEmpty)
        XCTAssertEqual(first.repositories.first?.tasks.map(\.work.id), ["b"])
        XCTAssertEqual(second.repositories.first?.tasks.map(\.work.id), ["b"])
        XCTAssertEqual(route?.id, "b")
        XCTAssertEqual(otherRoute?.id, "a")
        second.stop()
        first.filter = .available
        first.mount()
        XCTAssertEqual(first.repositories.first?.tasks.map(\.work.id), ["b"])
        first.filter = .started
        first.mount()
        owner.apply(prepared)
        XCTAssertEqual(first.repositories.first?.tasks.map(\.work.id), ["b"])
        XCTAssertEqual(route?.id, "b")
        hold.release()
        await owner.persistPending()
        XCTAssertEqual(first.repositories.first?.tasks.map(\.work.id), ["b"])
        first.complete(base.contents[1], selected: route, visible: [base.contents[1].work]) { route = $0 }
        XCTAssertNil(route)
        XCTAssertTrue(first.repositories.first?.tasks.isEmpty == true)
        await owner.persistPending()
        first.stop()
    }
}

import Foundation
import XCTest
import ViewInspector
@testable import Loopflow
@testable import LoopflowiOS

extension ClientWorkQueryTests {
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

    private func leaveBothFeeds(abandon: Bool, move: Bool = false) async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
        let query = ServedRead.viewSelect(expression: TaskList.started.expression(wave: nil))
        let base = try workingPlan(["a", "b"])
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
        _ = owner.open(otherQuery) { _ in }
        owner.apply(prepared)
        var entered = hold.entered.makeAsyncIterator()
        _ = await entered.next()
        var route: WorkWorkspaceKey? = base.contents[0].work
        let otherRoute = route
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

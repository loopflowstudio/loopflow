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

    private func leaveBothFeeds(abandon: Bool) async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
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
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { hold.release(); owner.shutdown() }
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

import Foundation
import Loopflow
import XCTest
@testable import LoopflowiOS

@MainActor
final class MobileTaskUpdatesTests: XCTestCase {
    func testReplacementFailureRecoveryAndSuspensionPreserveHonestState() async throws {
        let client = TaskClient(tasks: try await FixtureClient().workingTasks())
        let workspace = MobileWorkspace(client: client)
        await workspace.startTaskUpdates()
        try await wait { !workspace.tasks.isEmpty }
        let first = try XCTUnwrap(client.observations.first)
        first.continuation.yield(.failure(RegistryQueryError("Machine offline")))
        try await wait { workspace.tasksError == "Machine offline" }
        XCTAssertEqual(workspace.tasks.count, client.tasks.count, "Unavailable is not an empty list")
        first.continuation.yield(.success([]))
        try await wait { workspace.tasks.isEmpty && workspace.tasksError == nil }
        await workspace.refresh()
        try await wait { workspace.tasks.count == client.tasks.count }
        workspace.suspend()
        XCTAssertTrue(first.closed)
        XCTAssertFalse(workspace.isLoadingTasks)
        first.continuation.yield(.success([]))
        await workspace.startTaskUpdates()
        try await wait { !workspace.tasks.isEmpty }
        XCTAssertEqual(client.observations.count, 2)
        workspace.stopTaskUpdates()
        XCTAssertTrue(client.observations.allSatisfy(\.closed))
    }

    func testAnOlderOpeningCannotReplaceTheCurrentSubscription() async throws {
        let client = TaskClient(tasks: try await FixtureClient().workingTasks())
        client.delayFirst = true
        let workspace = MobileWorkspace(client: client)
        let opening = Task { await workspace.startTaskUpdates() }
        try await wait { client.releaseFirst != nil }
        await workspace.startTaskUpdates()
        try await wait { !workspace.tasks.isEmpty }
        client.releaseFirst?.resume()
        client.releaseFirst = nil
        await opening.value
        XCTAssertTrue(client.observations[0].closed)
        XCTAssertFalse(client.observations[1].closed)
        XCTAssertEqual(workspace.tasks.count, client.tasks.count)
        workspace.stopTaskUpdates()
    }

    func testStreamFailureKeepsTasksAndRefreshOpensANewWatch() async throws {
        let client = TaskClient(tasks: try await FixtureClient().workingTasks())
        let workspace = MobileWorkspace(client: client)
        await workspace.startTaskUpdates()
        try await wait { !workspace.tasks.isEmpty }
        client.observations[0].continuation.finish(throwing: RegistryQueryError("Connection lost"))
        try await wait { workspace.tasksError == "Connection lost" }
        XCTAssertEqual(workspace.tasks.count, client.tasks.count)
        await workspace.refresh()
        try await wait { workspace.tasksError == nil && !workspace.isLoadingTasks }
        XCTAssertEqual(client.observations.count, 2)
        workspace.stopTaskUpdates()
    }

    func testWatchFramesRejectWrongGenerationMissingContentsAndProjectionGaps() async throws {
        let rows = try await FixtureClient().workingTasks()
        let fixtureURL = try XCTUnwrap(Bundle.main.url(forResource: "task-reading", withExtension: "json"))
        let fixture = try JSONDecoder().decode(MachineResponse.self, from: Data(contentsOf: fixtureURL))
        guard case .read(let output) = fixture.result else { return XCTFail("Expected fixture plan") }
        let plan = try JSONSerialization.jsonObject(with: Data(output.utf8))
        func response(generation: Int = 1, body: Any = NSNull(), unavailable: Any = NSNull()) throws -> MachineResponse {
            let object: [String: Any] = ["id":"tasks-1", "instance":"test", "result":[
                "kind":"reading", "generation":generation,
                "frame":["answers":7, "unavailable":unavailable, "body":body],
            ]]
            return try JSONDecoder().decode(MachineResponse.self, from: JSONSerialization.data(withJSONObject: object))
        }
        let reading = try RemoteTaskObservation.decode(response(body: [plan]))
        XCTAssertEqual(reading.answers, 7)
        XCTAssertEqual(try reading.reading.get().map(\.work), rows.map(\.work))
        XCTAssertThrowsError(try RemoteTaskObservation.decode(response(generation: 2, body: [plan])))
        XCTAssertThrowsError(try RemoteTaskObservation.decode(response()))
        XCTAssertThrowsError(try RemoteTaskObservation.decode(response(unavailable: "Offline")).reading.get())
        var incomplete = try XCTUnwrap(plan as? [String: Any])
        incomplete["contents"] = []
        XCTAssertThrowsError(try RemoteTaskObservation.decode(response(body: [incomplete])).reading.get())
        let bundle = Bundle(for: Self.self)
        let url = try XCTUnwrap(bundle.url(forResource: "all_view_plans", withExtension: "json", subdirectory: "machine"))
        let request = MachineRequest(version: 1, id: "tasks-1",
            operation: .subscribe(.allViewPlans(expression: "kind:task state:open"), generation: 1))
        XCTAssertEqual(try JSONDecoder().decode(JSONValue.self, from: JSONEncoder().encode(request)),
            try JSONDecoder().decode(JSONValue.self, from: Data(contentsOf: url)))
    }

    private func wait(_ condition: () -> Bool) async throws {
        let end = ContinuousClock.now.advanced(by: .seconds(3))
        while !condition(), ContinuousClock.now < end { await Task.yield() }
        XCTAssertTrue(condition())
    }
}

@MainActor
private final class TaskClient: MobileClient {
    let tasks: [WorkViewPlan.Content]
    var observations: [FixtureTaskObservation] = []
    var delayFirst = false
    var releaseFirst: CheckedContinuation<Void, Never>?
    init(tasks: [WorkViewPlan.Content]) { self.tasks = tasks }
    func workingTasks() async throws -> [WorkViewPlan.Content] { tasks }
    func observeTasks() async throws -> any MobileTaskObservation {
        let observation = FixtureTaskObservation(tasks: tasks)
        observations.append(observation)
        if delayFirst && observations.count == 1 {
            await withCheckedContinuation { releaseFirst = $0 }
        }
        return observation
    }
    func attach(sessionID: String) async throws -> any MobileAttachment { throw SessionUnavailable("Not part of this test") }
}

import Foundation
import Loopflow
import SwiftUI
import UIKit
import ViewInspector
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

    func testReopeningOfflineUsesOnlyThatMachinesSavedTasksUntilALiveReplacement() async throws {
        let url = FileManager.default.temporaryDirectory.appending(path: UUID().uuidString).appending(path: "tasks.json")
        defer { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }
        let cache = MobileTaskCache(url: url)
        let rows = try await FixtureClient().workingTasks()
        let client = TaskClient(tasks: rows)
        let first = MobileWorkspace(client: client, machineID: "machine-a", taskCache: cache)
        await first.startTaskUpdates()
        try await wait { first.tasksAreCurrent }
        let saved = try await cache.load(machineID: "machine-a")
        XCTAssertEqual(saved?.tasks, rows)
        XCTAssertEqual(saved?.receivedAt, first.tasksReceivedAt)
        first.stopTaskUpdates()

        client.unavailable = true
        let reopened = MobileWorkspace(client: client, machineID: "machine-a", taskCache: cache)
        await reopened.startTaskUpdates()
        XCTAssertEqual(reopened.tasks, rows)
        XCTAssertEqual(reopened.tasksReceivedAt, saved?.receivedAt)
        XCTAssertFalse(reopened.tasksAreCurrent)
        XCTAssertFalse(reopened.isLoadingTasks)
        XCTAssertEqual(reopened.tasksError, "Machine offline")
        await reopened.open(try XCTUnwrap(rows.first))
        XCTAssertEqual(reopened.state, .unavailable("Offline Session"), "A cached Task still needs a live Session connection")

        let otherMachine = MobileWorkspace(client: client, machineID: "machine-b", taskCache: cache)
        await otherMachine.startTaskUpdates()
        XCTAssertTrue(otherMachine.tasks.isEmpty)
        XCTAssertNil(otherMachine.tasksReceivedAt)

        client.unavailable = false
        client.tasks = []
        await reopened.refresh()
        try await wait { reopened.tasksAreCurrent }
        XCTAssertTrue(reopened.tasks.isEmpty)
        XCTAssertNil(reopened.tasksError)
        let empty = try await cache.load(machineID: "machine-a")
        XCTAssertEqual(empty?.tasks, [])
        reopened.stopTaskUpdates()
        client.unavailable = true
        let again = MobileWorkspace(client: client, machineID: "machine-a", taskCache: cache)
        await again.startTaskUpdates()
        XCTAssertTrue(again.tasks.isEmpty, "Completed Tasks cannot reappear from an older cache")
        XCTAssertNotNil(again.tasksReceivedAt)
    }

    func testDisposableCacheRejectsDamagedOversizedAndOtherMachineReadings() async throws {
        let directory = FileManager.default.temporaryDirectory.appending(path: UUID().uuidString)
        let url = directory.appending(path: "tasks.json")
        defer { try? FileManager.default.removeItem(at: directory) }
        let cache = MobileTaskCache(url: url)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        for data in [Data("interrupted JSON".utf8), Data(repeating: 0, count: MobileTaskCache.maximumBytes + 1)] {
            try data.write(to: url)
            let reading = try await cache.load(machineID: "machine-a")
            XCTAssertNil(reading)
            XCTAssertFalse(FileManager.default.fileExists(atPath: url.path))
        }
        try await cache.save(.init(machineID: "machine-a", receivedAt: Date(), tasks: []))
        let different = try await cache.load(machineID: "machine-b")
        XCTAssertNil(different)
        try await cache.remove(machineID: "machine-b")
        let preserved = try await cache.load(machineID: "machine-a")
        XCTAssertNotNil(preserved, "Forgetting another Machine cannot delete this reading")
        let canceled = Task { try await cache.save(.init(machineID: "machine-b", receivedAt: Date(), tasks: [])) }
        canceled.cancel()
        do {
            try await canceled.value
            XCTFail("Canceled subscriptions cannot write a late snapshot")
        } catch { XCTAssertTrue(error is CancellationError) }
        let stillPreserved = try await cache.load(machineID: "machine-a")
        XCTAssertNotNil(stillPreserved)
        XCTAssertEqual(try url.resourceValues(forKeys: [.isExcludedFromBackupKey]).isExcludedFromBackup, true)
        try await cache.remove(machineID: "machine-a")
        let removed = try await cache.load(machineID: "machine-a")
        XCTAssertNil(removed)

        // An unwritable cache cannot turn a valid live reading into a failure.
        try Data("not a directory".utf8).write(to: url)
        let broken = MobileTaskCache(url: url.appending(path: "tasks.json"))
        let client = TaskClient(tasks: try await FixtureClient().workingTasks())
        let workspace = MobileWorkspace(client: client, machineID: "machine-a", taskCache: broken)
        await workspace.startTaskUpdates()
        try await wait { workspace.tasksAreCurrent }
        XCTAssertEqual(workspace.tasks, client.tasks)
        XCTAssertNil(workspace.tasksError)
        workspace.stopTaskUpdates()
    }

    func testDeviceCacheUsesCompleteFileProtection() async throws {
        #if targetEnvironment(simulator)
        throw XCTSkip("Simulator does not expose iOS file protection attributes; verify on a physical device")
        #else
        let url = FileManager.default.temporaryDirectory.appending(path: UUID().uuidString).appending(path: "tasks.json")
        defer { try? FileManager.default.removeItem(at: url.deletingLastPathComponent()) }
        let cache = MobileTaskCache(url: url)
        try await cache.save(.init(machineID: "machine-a", receivedAt: Date(), tasks: []))
        let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
        XCTAssertEqual(attributes[.protectionKey] as? String, FileProtectionType.complete.rawValue)
        #endif
    }

    func testSavedListStatusKeepsItsTimestampAndRecoveryVisible() async throws {
        let receivedAt = Date(timeIntervalSince1970: 1_793_700_000)
        let status = MobileTaskListStatus(error: "Machine offline", receivedAt: receivedAt,
                                          isLoading: false, refresh: {})
        XCTAssertNoThrow(try status.inspect().find(text: "Last updated \(receivedAt.formatted(date: .abbreviated, time: .shortened))"))
        XCTAssertFalse(try status.inspect().find(button: "Try again").isDisabled())
        let refreshing = MobileTaskListStatus(error: "Machine offline", receivedAt: receivedAt,
                                              isLoading: true, refresh: {})
        XCTAssertTrue(try refreshing.inspect().find(button: "Try again").isDisabled())
        for (name, style, size) in [("light", ColorScheme.light, DynamicTypeSize.large),
                                    ("dark", .dark, .large), ("large-text", .light, .accessibility3)] {
            let palette = style == .dark ? ThemeDefinition.dark.palette : ThemeDefinition.light.palette
            let view = NavigationStack {
                List { status.listRowBackground(palette.surface) }
                    .scrollContentBackground(.hidden).background(palette.background)
                    .foregroundStyle(palette.textSecondary).tint(palette.accentInk)
                    .navigationTitle("Tasks")
            }.environment(\.colorScheme, style).environment(\.dynamicTypeSize, size)
            let controller = UIHostingController(rootView: view)
            let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
            let previous = scene.keyWindow
            let window = UIWindow(windowScene: scene)
            window.frame = CGRect(x: 0, y: 0, width: 390, height: 844)
            window.rootViewController = controller
            window.makeKeyAndVisible()
            defer { window.isHidden = true; previous?.makeKey() }
            controller.view.frame = window.bounds
            controller.view.layoutIfNeeded()
            try await Task.sleep(for: .milliseconds(250))
            let image = UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
                XCTAssertTrue(window.drawHierarchy(in: window.bounds, afterScreenUpdates: true))
            }
            let attachment = XCTAttachment(image: image)
            attachment.name = "saved-task-list-status-\(name)"
            attachment.lifetime = .keepAlways
            add(attachment)
        }
    }

    func testWatchFramesRejectMissingContentsAndProjectionGaps() async throws {
        let rows = try await FixtureClient().workingTasks()
        let fixtureURL = try XCTUnwrap(Bundle.main.url(forResource: "task-reading", withExtension: "json"))
        guard case .response(_, _, let value) = try JSONDecoder().decode(ServerMessage.self, from: Data(contentsOf: fixtureURL)) else {
            return XCTFail("Expected fixture plan")
        }
        let plan = try JSONSerialization.jsonObject(with: JSONEncoder().encode(value))
        func frame(body: Any = NSNull(), unavailable: Any = NSNull()) throws -> JSONValue {
            let object: [String: Any] = ["answers": 7, "unavailable": unavailable, "body": body]
            return try JSONDecoder().decode(JSONValue.self, from: JSONSerialization.data(withJSONObject: object))
        }
        let reading = try RemoteTaskObservation.decode(frame: frame(body: [plan]))
        XCTAssertEqual(reading.answers, 7)
        XCTAssertEqual(try reading.reading.get().map(\.work), rows.map(\.work))
        XCTAssertThrowsError(try RemoteTaskObservation.decode(frame: frame()))
        XCTAssertThrowsError(try RemoteTaskObservation.decode(frame: frame(unavailable: "Offline")).reading.get())
        var incomplete = try XCTUnwrap(plan as? [String: Any])
        incomplete["contents"] = []
        XCTAssertThrowsError(try RemoteTaskObservation.decode(frame: frame(body: [incomplete])).reading.get())
        let bundle = Bundle(for: Self.self)
        let url = try XCTUnwrap(bundle.url(forResource: "view_select", withExtension: "json", subdirectory: "server"))
        let request = ServerClientMessage.subscribe(id: "tasks-1", operation: .allViewPlans(expression: "kind:task state:open"), select: nil)
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
    var tasks: [WorkViewPlan.Content]
    var unavailable = false
    var observations: [FixtureTaskObservation] = []
    var delayFirst = false
    var releaseFirst: CheckedContinuation<Void, Never>?
    init(tasks: [WorkViewPlan.Content]) { self.tasks = tasks }
    func workingTasks() async throws -> [WorkViewPlan.Content] { tasks }
    func observeTasks() async throws -> any MobileTaskObservation {
        if unavailable { throw RegistryQueryError("Machine offline") }
        let observation = FixtureTaskObservation(tasks: tasks)
        observations.append(observation)
        if delayFirst && observations.count == 1 {
            await withCheckedContinuation { releaseFirst = $0 }
        }
        return observation
    }
    func attach(sessionID: String) async throws -> any MobileAttachment { throw SessionUnavailable("Offline Session") }
}

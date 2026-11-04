import Foundation
import Loopflow
import SwiftUI
import UIKit
import XCTest
@testable import LoopflowiOS

@MainActor
final class MobileTaskCreationTests: XCTestCase {
    func testCreationSheetRendersInBothThemesAndLargeText() async throws {
        let client = try await CreationClient()
        let store = temporaryStore()
        defer { try? FileManager.default.removeItem(at: store.url.deletingLastPathComponent()) }
        let model = MobileTaskCreation(client: client, store: store)
        await model.prepare()
        model.editText("Make the Task sidebar easier to scan.")
        for (name, style, size) in [("light", ColorScheme.light, DynamicTypeSize.large),
                                    ("dark", .dark, .large), ("large-text", .light, .accessibility3)] {
            let view = MobileNewTaskView(model: model, theme: style == .dark ? .dark : .light, onOpen: { _ in })
                .environment(\.colorScheme, style).environment(\.dynamicTypeSize, size)
            let controller = UIHostingController(rootView: view)
            let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
            let previous = scene.keyWindow
            let window = UIWindow(windowScene: scene)
            window.frame = CGRect(x: 0, y: 0, width: 390, height: 844)
            window.rootViewController = controller
            window.makeKeyAndVisible()
            controller.view.frame = window.bounds
            controller.view.layoutIfNeeded()
            // Let SwiftUI install its navigation and safe-area layout.
            try await Task.sleep(for: .milliseconds(250))
            let image = UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
                XCTAssertTrue(window.drawHierarchy(in: window.bounds, afterScreenUpdates: true))
            }
            let attachment = XCTAttachment(image: image)
            attachment.name = "new-task-\(name)"
            attachment.lifetime = .keepAlways
            add(attachment)
            window.isHidden = true
            previous?.makeKey()
        }
    }

    func testLostReplyAndAppRestartRecoverOriginalRequestAndOpenRecordedTask() async throws {
        let client = try await CreationClient()
        let store = temporaryStore()
        defer { try? FileManager.default.removeItem(at: store.url.deletingLastPathComponent()) }
        let model = MobileTaskCreation(client: client, store: store)
        model.editText("Fix the keyboard\nKeep dictation editable")
        await model.prepare()
        XCTAssertTrue(model.canCreate)
        client.loseReply = true
        await model.submit()
        let original = try XCTUnwrap(model.draft.submission)
        XCTAssertNotNil(model.error)
        XCTAssertEqual(try store.load().submission, original, "Intent must survive loss of the command reply")
        model.editText("Do something else")
        XCTAssertEqual(model.draft.text, original.request, "An uncertain payload cannot be edited")

        let restored = MobileTaskCreation(client: client, store: store)
        await restored.prepare()
        await restored.submit()
        XCTAssertEqual(restored.draft.outcome?.state, .started)
        XCTAssertEqual(client.requests, [original, original])
        client.readUnavailable = true
        let missing = await restored.openTask()
        XCTAssertNil(missing)
        XCTAssertEqual(try store.load().submission, original)
        client.readUnavailable = false
        let task = await restored.openTask()
        XCTAssertEqual(task?.primarySession, client.task.primarySession)
        XCTAssertNil(try store.load().submission)
        XCTAssertEqual(restored.draft.directory, original.directory)
    }

    func testFailedAndUnknownStartsKeepFiledTaskWithoutRepeatingCreation() async throws {
        for state in ["failed", "unknown"] {
            let client = try await CreationClient()
            client.outcome = try client.result(state)
            let store = temporaryStore()
            defer { try? FileManager.default.removeItem(at: store.url.deletingLastPathComponent()) }
            let model = MobileTaskCreation(client: client, store: store)
            await model.prepare()
            model.editText("Repair selection")
            await model.submit()
            let restored = MobileTaskCreation(client: client, store: store)
            XCTAssertTrue(restored.settled)
            await restored.submit()
            XCTAssertEqual(client.requests.count, 1)
            let opened = await restored.openTask()
            XCTAssertEqual(opened?.work, client.task.work)
        }
    }

    func testDismissedCreationCannotOverwriteARecoveredDraft() async throws {
        let client = try await CreationClient()
        let store = temporaryStore()
        defer { try? FileManager.default.removeItem(at: store.url.deletingLastPathComponent()) }
        let first = MobileTaskCreation(client: client, store: store)
        await first.prepare()
        first.editText("Keep the original request")
        client.delayReply = true
        let sending = Task { await first.submit() }
        while client.reply == nil { await Task.yield() }
        sending.cancel()
        let reopened = MobileTaskCreation(client: client, store: store)
        await reopened.prepare()
        await reopened.submit()
        let opened = await reopened.openTask()
        XCTAssertNotNil(opened)
        client.reply?.resume()
        await sending.value
        XCTAssertNil(try store.load().submission, "A dismissed sheet must not restore its old pending request")
        XCTAssertEqual(client.requests.first, client.requests.last)
    }

    func testDraftDestinationAndPendingMachineSurviveRePairing() async throws {
        let client = try await CreationClient()
        let store = temporaryStore()
        defer { try? FileManager.default.removeItem(at: store.url.deletingLastPathComponent()) }
        let model = MobileTaskCreation(client: client, store: store)
        await model.prepare()
        model.selectRepository("/work/second")
        model.editText("A saved idea")
        XCTAssertTrue(model.saveDraft())
        let restored = MobileTaskCreation(client: client, store: store)
        await restored.prepare()
        XCTAssertEqual(restored.draft.directory, "/work/second")
        XCTAssertEqual(restored.draft.text, "A saved idea")
        client.loseReply = true
        await restored.submit()
        let original = restored.draft.submission
        client.machineID = "machine-b"
        let rePaired = MobileTaskCreation(client: client, store: store)
        await rePaired.prepare()
        XCTAssertNotNil(rePaired.error)
        XCTAssertEqual(rePaired.draft.submission, original)
        await rePaired.submit()
        XCTAssertEqual(client.requests.count, 1, "Never send an earlier request to another Machine")
    }

    func testUnreadableDraftAndFailedSaveNeverSendCreation() async throws {
        let client = try await CreationClient()
        let store = temporaryStore()
        defer { try? FileManager.default.removeItem(at: store.url.deletingLastPathComponent()) }
        try FileManager.default.createDirectory(at: store.url.deletingLastPathComponent(), withIntermediateDirectories: true)
        let bad = Data("interrupted local file".utf8)
        try bad.write(to: store.url)
        let corrupted = MobileTaskCreation(client: client, store: store)
        await corrupted.prepare()
        corrupted.editText("Must not overwrite")
        await corrupted.submit()
        XCTAssertTrue(corrupted.storageUnavailable)
        XCTAssertEqual(try Data(contentsOf: store.url), bad)
        try FileManager.default.removeItem(at: store.url)
        let model = MobileTaskCreation(client: client, store: store)
        await model.prepare()
        model.editText("Don’t send without recovery")
        try FileManager.default.removeItem(at: store.url)
        try FileManager.default.createDirectory(at: store.url, withIntermediateDirectories: true)
        await model.submit()
        XCTAssertNotNil(model.error)
        XCTAssertTrue(client.requests.isEmpty)
    }

    private func temporaryStore() -> MobileTaskDraftStore {
        MobileTaskDraftStore(url: FileManager.default.temporaryDirectory.appending(path: UUID().uuidString).appending(path: "draft.json"))
    }
}

@MainActor
private final class CreationClient: MobileTaskCreationClient {
    let task: WorkViewPlan.Content
    var machineID = "machine-a"
    var requests: [MobileTaskSubmission] = []
    var loseReply = false
    var readUnavailable = false
    var delayReply = false
    var reply: CheckedContinuation<Void, Never>?
    var outcome: TaskCreateOutcome?

    init() async throws {
        let tasks = try await FixtureClient().workingTasks()
        task = try XCTUnwrap(tasks.first)
    }

    func creationContext() async throws -> MobileCreationContext {
        MobileCreationContext(machineID: machineID, name: "Development Mac", repositories: [
            PortfolioRepo(path: "/work/first", error: nil), PortfolioRepo(path: "/work/second", error: nil),
        ])
    }

    func createTask(_ submission: MobileTaskSubmission) async throws -> TaskCreateOutcome {
        guard submission.machineID == machineID else { throw RegistryQueryError("Different Machine") }
        requests.append(submission)
        if delayReply {
            delayReply = false
            await withCheckedContinuation { reply = $0 }
        }
        if loseReply { loseReply = false; throw RegistryQueryError("Reply lost after admission") }
        return try outcome ?? result("started")
    }

    func createdTask(_ submission: MobileTaskSubmission, taskID: String) async throws -> WorkViewPlan.Content {
        if readUnavailable { throw RegistryQueryError("Workspace not ready") }
        return task
    }

    func result(_ state: String) throws -> TaskCreateOutcome {
        let object: [String: Any] = ["task_id": task.work.id, "state": state,
            "receipt": ["task": ["id": task.work.id, "identifier": "LOO-TEST", "title": "Keyboard"],
                        "graph": "code", "session_id": task.primarySession as Any],
            "message": NSNull(), "owner": "owner-test"]
        return try JSONDecoder().decode(TaskCreateOutcome.self, from: JSONSerialization.data(withJSONObject: object))
    }
}

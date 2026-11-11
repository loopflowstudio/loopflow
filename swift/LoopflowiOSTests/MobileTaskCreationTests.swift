import Foundation
import Loopflow
import SwiftUI
import UIKit
import ViewInspector
import XCTest
@testable import LoopflowiOS

@MainActor
final class MobileTaskCreationTests: XCTestCase {
    func testSheetKeepsUnsavedInstructionsUntilStorageRecovers() async throws {
        let client = try await CreationClient()
        let store = temporaryStore()
        defer { try? FileManager.default.removeItem(at: store.url.deletingLastPathComponent()) }
        let model = MobileTaskCreation(client: client, store: store)
        await model.prepare()
        XCTAssertFalse(model.hasUnsavedChanges)
        model.editText("Keep this unsaved idea on screen")
        XCTAssertTrue(model.hasUnsavedChanges, "Protect edits before the autosave runs")
        try FileManager.default.removeItem(at: store.url)
        try FileManager.default.createDirectory(at: store.url, withIntermediateDirectories: true)
        XCTAssertFalse(model.saveDraft())
        let view = Color.clear.sheet(isPresented: .constant(true)) {
            MobileNewTaskView(model: model, theme: .light, onOpen: { _ in XCTFail("No Task was requested") })
        }
        try await capture(view, name: "new-task-storage-recovery") { window in
            let clock = ContinuousClock()
            let presentedDeadline = clock.now + .seconds(3)
            while window.rootViewController?.presentedViewController == nil && clock.now < presentedDeadline {
                try await Task.sleep(for: .milliseconds(10))
            }
            let sheet = try XCTUnwrap(window.rootViewController?.presentedViewController)
            let protectedDeadline = clock.now + .seconds(3)
            while !sheet.isModalInPresentation && clock.now < protectedDeadline {
                try await Task.sleep(for: .milliseconds(10))
            }
            XCTAssertTrue(sheet.isModalInPresentation, "A swipe must not discard instructions that failed to save")
            try FileManager.default.removeItem(at: store.url)
            XCTAssertTrue(model.saveDraft())
            let savedDeadline = clock.now + .seconds(3)
            while sheet.isModalInPresentation && clock.now < savedDeadline {
                try await Task.sleep(for: .milliseconds(10))
            }
            XCTAssertFalse(sheet.isModalInPresentation, "Saving restores ordinary sheet dismissal")
            XCTAssertEqual(try store.load().text, "Keep this unsaved idea on screen")
            XCTAssertFalse(MobileTaskCreation(client: client, store: store).hasUnsavedChanges)
            XCTAssertTrue(client.requests.isEmpty)
        }
    }

    func testTaskReadKeepsRecoveryAcrossRestartUntilPresentation() async throws {
        let client = try await CreationClient()
        let store = temporaryStore()
        defer { try? FileManager.default.removeItem(at: store.url.deletingLastPathComponent()) }
        let model = MobileTaskCreation(client: client, store: store)
        await model.prepare()
        model.editText("Keep this until I can see the Task")
        await model.submit()
        let original = try XCTUnwrap(model.draft.submission)
        let opening = await model.openTask()
        XCTAssertEqual(opening?.task.work, client.task.work)
        let restored = MobileTaskCreation(client: client, store: store)
        XCTAssertEqual(restored.draft.submission, original)
        XCTAssertEqual(restored.draft.outcome, model.draft.outcome)
        opening?.presented()
        XCTAssertNil(try store.load().submission)
        XCTAssertEqual(client.requests.count, 1)
    }

    func testLateAndRepeatedPresentationPreserveANewerDraft() async throws {
        let client = try await CreationClient()
        let store = temporaryStore()
        defer { try? FileManager.default.removeItem(at: store.url.deletingLastPathComponent()) }
        let first = MobileTaskCreation(client: client, store: store)
        await first.prepare()
        first.editText("Original work")
        await first.submit()
        let read = await first.openTask()
        let opening = try XCTUnwrap(read)
        let newer = MobileTaskCreation(client: client, store: store)
        XCTAssertTrue(newer.saveForLater())
        newer.editText("Do not replace this newer idea")
        newer.selectRepository("/work/second")
        XCTAssertTrue(newer.saveDraft())
        opening.presented()
        opening.presented()
        let saved = try store.load()
        XCTAssertEqual(saved.text, "Do not replace this newer idea")
        XCTAssertEqual(saved.directory, "/work/second")
        XCTAssertTrue(saved.savedRequests.isEmpty)
        XCTAssertNil(saved.submission)
    }

    func testCanceledTaskReadCannotOpenOrDiscardRecovery() async throws {
        let client = try await CreationClient()
        let store = temporaryStore()
        defer { try? FileManager.default.removeItem(at: store.url.deletingLastPathComponent()) }
        let model = MobileTaskCreation(client: client, store: store)
        await model.prepare()
        model.editText("Cancel navigation, keep the created Task")
        await model.submit()
        let original = model.draft.submission
        client.delayRead = true
        let reading = Task { await model.openTask() }
        while client.readReply == nil { await Task.yield() }
        reading.cancel()
        client.readReply?.resume()
        let opened = await reading.value
        XCTAssertNil(opened)
        XCTAssertEqual(try store.load().submission, original)
    }

    func testRepositoryRefreshAndSelectionPreserveInstructionsAndSavedDestination() async throws {
        let client = try await CreationClient()
        client.repositories = []
        let store = temporaryStore()
        defer { try? FileManager.default.removeItem(at: store.url.deletingLastPathComponent()) }
        let model = MobileTaskCreation(client: client, store: store)
        model.editText("Keep this idea while I connect a repository")
        await model.prepare()
        XCTAssertFalse(model.canCreate)

        client.repositories = [PortfolioRepo(path: "/work/first", error: nil),
                               PortfolioRepo(path: "/archive/first", error: nil),
                               PortfolioRepo(path: "/offline/first", error: "Repository is unavailable")]
        await model.prepare()
        let picker = MobileRepositoryPicker(model: model, theme: .light)
        let unavailable = try picker.inspect().find(text: "/offline/first").find(ViewType.Button.self, relation: .parent)
        XCTAssertTrue(unavailable.isDisabled())
        try picker.inspect().find(text: "/archive/first").find(ViewType.Button.self, relation: .parent).tap()
        XCTAssertEqual(try store.load().directory, "/archive/first", "Equal names must still select the exact path")
        XCTAssertEqual(try store.load().text, "Keep this idea while I connect a repository")
        XCTAssertTrue(model.canCreate)

        client.repositories = [PortfolioRepo(path: "/work/first", error: nil)]
        await model.prepare()
        XCTAssertEqual(model.draft.directory, "/archive/first", "Refresh cannot silently retarget a saved draft")
        XCTAssertFalse(model.canCreate)
        client.contextFailure = "Machine went offline"
        await model.prepare()
        XCTAssertEqual(model.error, "Machine went offline")
        XCTAssertEqual(try store.load().directory, "/archive/first")
        XCTAssertEqual(model.draft.text, "Keep this idea while I connect a repository")
        XCTAssertTrue(client.requests.isEmpty)
    }

    func testRepositoryPickerRendersPathsErrorsAndEmptyState() async throws {
        let client = try await CreationClient()
        client.repositories = [
            PortfolioRepo(path: "/Users/developer/src/loopflow", error: nil),
            PortfolioRepo(path: "/Users/developer/experiments/loopflow", error: nil),
            PortfolioRepo(path: "/Volumes/Projects/website", error: "This repository is unavailable on the Machine."),
        ]
        let store = temporaryStore()
        defer { try? FileManager.default.removeItem(at: store.url.deletingLastPathComponent()) }
        let model = MobileTaskCreation(client: client, store: store)
        await model.prepare()
        for (name, style, size) in [("light", ColorScheme.light, DynamicTypeSize.large),
                                    ("dark", .dark, .large), ("large-text", .light, .accessibility3)] {
            let view = NavigationStack {
                MobileRepositoryPicker(model: model, theme: style == .dark ? .dark : .light)
            }.environment(\.colorScheme, style).environment(\.dynamicTypeSize, size)
            try await capture(view, name: "repository-picker-\(name)")
        }
        try await capture(NavigationStack { MobileRepositoryPicker(model: model, theme: .light) },
                          name: "repository-picker-search") { window in
            let search = try XCTUnwrap(self.searchField(in: window))
            XCTAssertTrue(search.becomeFirstResponder())
            search.insertText("experiments")
            try await Task.sleep(for: .milliseconds(250))
        }
        client.repositories = []
        await model.prepare()
        try await capture(NavigationStack { MobileRepositoryPicker(model: model, theme: .light) },
                          name: "repository-picker-empty")
    }

    func testSavedRequestRecoversAfterRestartWithoutLosingNewDraft() async throws {
        let client = try await CreationClient()
        let store = temporaryStore()
        defer { try? FileManager.default.removeItem(at: store.url.deletingLastPathComponent()) }
        let model = MobileTaskCreation(client: client, store: store)
        await model.prepare()
        model.editText("Original work after a lost reply")
        client.loseReply = true
        await model.submit()
        let original = try XCTUnwrap(model.draft.submission)
        XCTAssertTrue(model.saveForLater())
        XCTAssertFalse(model.saveForLater(), "Saving twice cannot duplicate recovery records")
        XCTAssertNil(model.draft.submission)
        XCTAssertEqual(model.savedRequest(original.id)?.submission, original)
        model.editText("A different idea, still being edited")
        model.selectRepository("/work/second")
        XCTAssertTrue(model.canCreate)
        XCTAssertTrue(model.saveDraft())

        let restored = MobileTaskCreation(client: client, store: store)
        await restored.prepare()
        await restored.submit(savedRequestID: original.id)
        XCTAssertEqual(client.requests, [original, original])
        XCTAssertEqual(restored.savedRequest(original.id)?.outcome?.state, .started)
        let opened = await restored.openTask(savedRequestID: original.id)
        XCTAssertEqual(opened?.task.work, client.task.work)
        opened?.presented()
        let remaining = try store.load()
        XCTAssertTrue(remaining.savedRequests.isEmpty)
        XCTAssertEqual(remaining.text, "A different idea, still being edited")
        XCTAssertEqual(remaining.directory, "/work/second")
        XCTAssertNil(remaining.submission)
        await restored.submit(savedRequestID: original.id)
        XCTAssertEqual(client.requests.count, 2, "A removed request cannot fall back to submitting the new draft")
    }

    func testOlderSavedReceiptKeepsItsSubmissionAndTaskWithoutInventingAMilestone() async throws {
        let client = try await CreationClient()
        let store = temporaryStore()
        defer { try? FileManager.default.removeItem(at: store.url.deletingLastPathComponent()) }
        let model = MobileTaskCreation(client: client, store: store)
        await model.prepare()
        model.editText("Keep my original request")
        await model.submit()
        let submission = try XCTUnwrap(model.draft.submission)
        var saved = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: store.url)) as? [String: Any])
        var outcome = try XCTUnwrap(saved["outcome"] as? [String: Any])
        var receipt = try XCTUnwrap(outcome["receipt"] as? [String: Any])
        receipt.removeValue(forKey: "milestone")
        outcome["receipt"] = receipt
        saved["outcome"] = outcome
        try JSONSerialization.data(withJSONObject: saved).write(to: store.url)
        let restored = MobileTaskCreation(client: client, store: store)
        XCTAssertFalse(restored.storageUnavailable)
        XCTAssertEqual(restored.draft.submission, submission)
        XCTAssertEqual(restored.draft.outcome?.taskId, client.task.work.id)
        XCTAssertNil(restored.draft.outcome?.receipt)
        let opened = await restored.openTask()
        XCTAssertEqual(opened?.task.work, client.task.work)
        XCTAssertEqual(client.requests.count, 1, "Opening the existing Task must not repeat creation")
    }

    func testSavingOriginalMachineRequestAllowsNewWorkWithoutRetargetingRecovery() async throws {
        let client = try await CreationClient()
        let store = temporaryStore()
        defer { try? FileManager.default.removeItem(at: store.url.deletingLastPathComponent()) }
        let model = MobileTaskCreation(client: client, store: store)
        await model.prepare()
        model.editText("Work on the first Machine")
        client.loseReply = true
        await model.submit()
        let original = try XCTUnwrap(model.draft.submission)
        client.machineID = "machine-b"
        await model.prepare()
        XCTAssertNotNil(model.error)
        XCTAssertTrue(model.saveForLater())
        XCTAssertNil(model.error)
        model.editText("Different work on the second Machine")
        await model.submit()
        let second = try XCTUnwrap(model.draft.submission)
        XCTAssertEqual(second.machineID, "machine-b")
        XCTAssertNotEqual(original.id, second.id)
        XCTAssertEqual(model.savedRequest(original.id)?.submission, original)
        await model.submit(savedRequestID: original.id)
        XCTAssertEqual(client.requests, [original, second], "Recovery cannot go to the newly paired Machine")
        XCTAssertNil(model.savedRequest(original.id)?.outcome)
        let opened = await model.openTask()
        XCTAssertNotNil(opened)
        opened?.presented()
        XCTAssertEqual(try store.load().savedRequests.first?.submission, original)
    }

    func testSavingForLaterAndRecoveryKeepRequestOnLocalWriteFailure() async throws {
        let client = try await CreationClient()
        let store = temporaryStore()
        defer { try? FileManager.default.removeItem(at: store.url.deletingLastPathComponent()) }
        let model = MobileTaskCreation(client: client, store: store)
        await model.prepare()
        model.editText("Keep this recoverable")
        await model.submit()
        let original = try XCTUnwrap(model.draft.submission)
        let bytes = try Data(contentsOf: store.url)
        try FileManager.default.removeItem(at: store.url)
        try FileManager.default.createDirectory(at: store.url, withIntermediateDirectories: true)
        XCTAssertFalse(model.saveForLater())
        XCTAssertEqual(model.draft.submission, original)
        XCTAssertTrue(model.draft.savedRequests.isEmpty)
        try FileManager.default.removeItem(at: store.url)
        try bytes.write(to: store.url)
        XCTAssertTrue(model.saveForLater())
        client.readUnavailable = true
        let missing = await model.openTask(savedRequestID: original.id)
        XCTAssertNil(missing)
        XCTAssertEqual(try store.load().savedRequests.first?.submission, original)
    }

    func testCanceledSavedRecoveryCannotRestoreAnAlreadyOpenedRequest() async throws {
        let client = try await CreationClient()
        let store = temporaryStore()
        defer { try? FileManager.default.removeItem(at: store.url.deletingLastPathComponent()) }
        let model = MobileTaskCreation(client: client, store: store)
        await model.prepare()
        model.editText("Recover once")
        client.loseReply = true
        await model.submit()
        let original = try XCTUnwrap(model.draft.submission)
        XCTAssertTrue(model.saveForLater())
        client.delayReply = true
        let checking = Task { await model.submit(savedRequestID: original.id) }
        while client.reply == nil { await Task.yield() }
        checking.cancel()
        let restored = MobileTaskCreation(client: client, store: store)
        await restored.submit(savedRequestID: original.id)
        let opened = await restored.openTask(savedRequestID: original.id)
        XCTAssertNotNil(opened)
        opened?.presented()
        client.reply?.resume()
        await checking.value
        XCTAssertTrue(try store.load().savedRequests.isEmpty)
    }

    func testExistingLocalDraftGainsEmptyRecoveryList() throws {
        let draft = try JSONDecoder().decode(MobileTaskDraft.self,
            from: Data(#"{"text":"An unfinished idea","directory":"/work/first","machineID":"machine-a"}"#.utf8))
        XCTAssertEqual(draft.text, "An unfinished idea")
        XCTAssertTrue(draft.savedRequests.isEmpty)
    }

    func testSaveForLaterButtonAndSavedRecoveryScreens() async throws {
        let client = try await CreationClient()
        let store = temporaryStore()
        defer { try? FileManager.default.removeItem(at: store.url.deletingLastPathComponent()) }
        let model = MobileTaskCreation(client: client, store: store)
        await model.prepare()
        model.editText("Make the Task sidebar easier to scan, preserving the work already running.")
        client.loseReply = true
        await model.submit()
        let id = try XCTUnwrap(model.draft.submission?.id)
        let composer = MobileNewTaskView(model: model, theme: .light, onOpen: { _ in })
        try composer.inspect().find(button: "Save for later").tap()
        XCTAssertNil(model.draft.submission)
        model.editText("An unrelated draft must remain here")
        for (name, style, size) in [("light", ColorScheme.light, DynamicTypeSize.large),
                                    ("dark", .dark, .large), ("large-text", .light, .accessibility3)] {
            let theme = style == .dark ? ThemeDefinition.dark : .light
            let list = NavigationStack {
                MobileSavedRequestsView(model: model, theme: theme, onOpen: { _ in })
            }.environment(\.colorScheme, style).environment(\.dynamicTypeSize, size)
            try await capture(list, name: "saved-requests-\(name)")
            let detail = NavigationStack {
                MobileSavedTaskRequestView(id: id, model: model, theme: theme, onOpen: { _ in })
            }.environment(\.colorScheme, style).environment(\.dynamicTypeSize, size)
            try await capture(detail, name: "saved-request-\(name)")
        }
        XCTAssertEqual(model.draft.text, "An unrelated draft must remain here")
        XCTAssertEqual(client.requests.count, 1, "Opening saved screens must never resend a request")
        var opened: MobileTaskOpening?
        let detail = MobileSavedTaskRequestView(id: id, model: model, theme: .light) { opened = $0 }
        try detail.inspect().find(button: "Check status").tap()
        let deadline = Date().addingTimeInterval(2)
        while opened == nil && Date() < deadline { try await Task.sleep(for: .milliseconds(10)) }
        XCTAssertEqual(opened?.task.work, client.task.work)
        XCTAssertNotNil(model.savedRequest(id), "A read is not presentation")
        opened?.presented()
        XCTAssertEqual(model.draft.text, "An unrelated draft must remain here")
        XCTAssertNil(model.savedRequest(id))
        XCTAssertEqual(client.requests.first, client.requests.last)
    }

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
            try await capture(view, name: "new-task-\(name)")
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
        XCTAssertEqual(task?.task.primarySession, client.task.primarySession)
        XCTAssertEqual(try store.load().submission, original)
        task?.presented()
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
            XCTAssertEqual(opened?.task.work, client.task.work)
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
        opened?.presented()
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

    private func capture<Content: View>(_ view: Content, name: String,
                                       interact: ((UIWindow) async throws -> Void)? = nil) async throws {
        let controller = UIHostingController(rootView: view)
        let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
        let previous = scene.keyWindow
        let window = UIWindow(windowScene: scene)
        window.frame = CGRect(x: 0, y: 0, width: 390, height: 844)
        window.rootViewController = controller
        window.makeKeyAndVisible()
        defer {
            window.isHidden = true
            previous?.makeKey()
        }
        controller.view.frame = window.bounds
        controller.view.layoutIfNeeded()
        // Let SwiftUI install its navigation and safe-area layout.
        try await Task.sleep(for: .milliseconds(250))
        try await interact?(window)
        let image = UIGraphicsImageRenderer(bounds: window.bounds).image { _ in
            XCTAssertTrue(window.drawHierarchy(in: window.bounds, afterScreenUpdates: true))
        }
        let attachment = XCTAttachment(image: image)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func searchField(in view: UIView) -> UITextField? {
        if let search = view as? UITextField { return search }
        return view.subviews.lazy.compactMap { self.searchField(in: $0) }.first
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
    var delayRead = false
    var readReply: CheckedContinuation<Void, Never>?
    var outcome: MobileTaskOutcome?
    var repositories = [PortfolioRepo(path: "/work/first", error: nil), PortfolioRepo(path: "/work/second", error: nil)]
    var contextFailure: String?

    init() async throws {
        let url = try XCTUnwrap(Bundle(for: MobileTaskCreationTests.self).url(forResource: "task-reading", withExtension: "json"))
        guard case .response(_, _, let result) = try JSONDecoder().decode(ServerMessage.self, from: Data(contentsOf: url)) else {
            throw RegistryQueryError("task-reading.json is not a response")
        }
        let plan = try JSONDecoder().decode(WorkViewPlan.self, from: JSONEncoder().encode(result))
        task = try XCTUnwrap(mobileTasks(plan).first)
    }

    func creationContext() async throws -> MobileCreationContext {
        if let contextFailure { throw RegistryQueryError(contextFailure) }
        return MobileCreationContext(machineID: machineID, name: "Development Mac", repositories: repositories)
    }

    func createTask(_ submission: MobileTaskSubmission) async throws -> MobileTaskOutcome {
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
        guard submission.machineID == machineID else { throw RegistryQueryError("Different Machine") }
        if readUnavailable { throw RegistryQueryError("Workspace not ready") }
        if delayRead {
            delayRead = false
            await withCheckedContinuation { readReply = $0 }
        }
        return task
    }

    func result(_ state: String) throws -> MobileTaskOutcome {
        let object: [String: Any] = ["task_id": task.work.id, "state": state,
            "receipt": ["task": ["id": task.work.id, "identifier": "LOO-TEST", "title": "Keyboard"],
                        "milestone": "admitted", "graph": "code", "session_id": task.primarySession as Any],
            "message": NSNull()]
        return try JSONDecoder().decode(MobileTaskOutcome.self, from: JSONSerialization.data(withJSONObject: object))
    }
}

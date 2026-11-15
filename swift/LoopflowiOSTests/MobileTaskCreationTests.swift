import Foundation
import Observation
@testable import Loopflow
import ViewInspector
import XCTest
@testable import LoopflowiOS

@MainActor
final class MobileTaskCreationTests: XCTestCase {
    func testCreateButtonSelectsSubmittedWordsBeforeHeldSaveAndClosesEditor() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let hold = HeldIntentDisk()
        let disk = ClientWorkStore(directory: root.appendingPathComponent("outbox"), prepareDirectory: { _ in hold.enter() },
            machine: { "machine" }, read: { _ in "null" },
            observe: { _ in ServerWatch(updates: AsyncThrowingStream { _ in }, checkpoint: { _ in }, cancel: {}) },
            send: { _ in XCTFail("Held save must not send") })
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { hold.release(); owner.shutdown() }
        let client = EditorClient(interaction: owner, disk: disk)
        let store = MobileTaskDraftStore(url: root.appendingPathComponent("draft.json"))
        let model = MobileTaskCreation(client: client, store: store)
        await model.prepare()
        model.editText("  Original words\n")
        XCTAssertTrue(model.saveDraft())
        var selected: CreationOperation?
        var editorOpen = true
        let view = MobileNewTaskView(model: model, theme: .light) { operation in
            selected = operation
            editorOpen = false
        }
        try view.inspect().find(viewWithAccessibilityIdentifier: "task-create").button().tap()
        XCTAssertFalse(editorOpen)
        XCTAssertEqual(selected?.request, "  Original words\n")
        XCTAssertEqual(selected?.directory, "/repo")
        XCTAssertNil(selected?.receipt)
        var entered = hold.entered.makeAsyncIterator()
        _ = await entered.next()
        XCTAssertEqual(try store.load().text, "  Original words\n", "Do not discard editor words before save commits")
        hold.release()
        await owner.persistPending()
        let saved = try await disk.pending()
        XCTAssertEqual(saved.first?.id, selected?.id)
        XCTAssertEqual(try JSONDecoder().decode(JSONValue.self, from: XCTUnwrap(saved.first).payload)["request"], .string("  Original words\n"))
    }

    func testLegacyFileImportsBeforeRewriteAndKeepsNewerUnsentDraft() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let disk = ClientWorkStore(directory: root.appendingPathComponent("outbox"), machine: { "machine" }, read: { _ in "null" },
            observe: { _ in ServerWatch(updates: AsyncThrowingStream { _ in }, checkpoint: { _ in }, cancel: {}) },
            send: { _ in XCTFail("Import cannot send") })
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { owner.shutdown() }
        let client = EditorClient(interaction: owner, disk: disk)
        let store = MobileTaskDraftStore(url: root.appendingPathComponent("draft.json"))
        let id = UUID()
        let source: [String: Any] = ["text": "Newer unsent words", "directory": "/repo", "machineID": "machine",
            "submission": ["id": id.uuidString, "machineID": "machine", "machineName": "Original Mac", "directory": "/repo", "request": "Submitted words"],
            "outcome": ["task_id": "original-task", "state": "unknown", "message": "Original outcome"], "savedRequests": []]
        let bytes = try JSONSerialization.data(withJSONObject: source)
        try bytes.write(to: store.url)
        client.importFailure = true
        let failed = MobileTaskCreation(client: client, store: store)
        await failed.prepare()
        XCTAssertEqual(try Data(contentsOf: store.url), bytes)
        XCTAssertFalse(failed.saveDraft(), "A failed import must not remove saved submission fields")
        client.importFailure = false
        await failed.prepare()
        XCTAssertEqual(try store.load().text, "Newer unsent words")
        let imported = try await disk.pending()
        XCTAssertEqual(imported.map(\.id), [id.uuidString.lowercased()])
        await owner.refreshFromStore()
        XCTAssertEqual(owner.creations.first?.taskID, "original-task")
        XCTAssertEqual(owner.creations.first?.request, "Submitted words")
        // Simulate a crash after the database committed but before the source rewrite.
        try bytes.write(to: store.url)
        let reopened = MobileTaskCreation(client: client, store: store)
        await reopened.prepare()
        let repeated = try await disk.pending()
        XCTAssertEqual(repeated.count, 1)
        XCTAssertEqual(try store.load().text, "Newer unsent words")
    }

    func testRestartAfterOutboxSaveDoesNotResubmitTheSurvivingEditorFile() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let disk = ClientWorkStore(directory: root.appendingPathComponent("outbox"), machine: { "machine" }, read: { _ in "null" },
            observe: { _ in ServerWatch(updates: AsyncThrowingStream { _ in }, checkpoint: { _ in }, cancel: {}) }, send: { _ in XCTFail() })
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { owner.shutdown() }
        let store = MobileTaskDraftStore(url: root.appendingPathComponent("draft.json"))
        var original = MobileTaskDraft()
        original.text = "Submitted words"
        original.directory = "/repo"
        original.machineID = "machine"
        try store.save(original)
        try await disk.save(SubmittedOperation(requestID: "original-operation", destination: "machine", payload: JSONEncoder().encode(
            ServedWrite.taskCreate(directory: "/repo", requestID: "original-operation", title: nil, request: original.text,
                wave: nil, graph: nil, session: nil, run: true)), version: ServerProtocol.revision, submittedAt: Date(), draftID: original.id))
        let restored = MobileTaskCreation(client: EditorClient(interaction: owner, disk: disk), store: store)
        await restored.prepare()
        XCTAssertEqual(restored.draft.text, "")
        XCTAssertNotEqual(restored.draft.id, original.id)
        XCTAssertFalse(restored.canCreate)
        let pending = try await disk.pending()
        XCTAssertEqual(pending.map(\.id), ["original-operation"])
    }

    func testImportDoesNotRewriteAnEditorSavedWhileItWaited() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let disk = ClientWorkStore(directory: root.appendingPathComponent("outbox"), machine: { "machine" }, read: { _ in "null" },
            observe: { _ in ServerWatch(updates: AsyncThrowingStream { _ in }, checkpoint: { _ in }, cancel: {}) }, send: { _ in XCTFail() })
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { owner.shutdown() }
        let client = EditorClient(interaction: owner, disk: disk)
        let store = MobileTaskDraftStore(url: root.appendingPathComponent("draft.json"))
        let source: [String: Any] = ["text": "Old words", "directory": "/repo", "machineID": "machine",
            "submission": ["id": UUID().uuidString, "machineID": "machine", "machineName": "Original Mac", "directory": "/repo", "request": "Old words"]]
        try JSONSerialization.data(withJSONObject: source).write(to: store.url)
        var newer = MobileTaskDraft()
        newer.text = "Newer unsent words"
        newer.directory = "/repo"
        newer.machineID = "machine"
        let newDraft = newer
        client.beforeImport = { try store.save(newDraft) }
        let editor = MobileTaskCreation(client: client, store: store)
        await editor.prepare()
        XCTAssertEqual(try store.load().text, "Newer unsent words")
        XCTAssertEqual(editor.draft.text, "Newer unsent words")
        let imported = try await disk.pending()
        XCTAssertEqual(imported.count, 1)
    }

    func testFiledOperationKeepsLaterSelectionAndNativeRetryUsesOnlyItsAdmittedSession() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let server = CreationMachine()
        let disk = ClientWorkStore(directory: root, machine: { "machine" }, read: { _ in "null" },
            observe: { server.observe($0) }, send: { server.send($0) })
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { owner.shutdown(); server.finish() }
        let client = RemoteMobileClient(savedMachine: SavedMachine(endpoint: URL(string: "https://original.invalid")!, machineID: "machine"))
        client.journal = disk
        client.interaction = owner
        let url = try XCTUnwrap(Bundle(for: Self.self).url(forResource: "task-reading", withExtension: "json"))
        guard case .response(_, _, let result) = try ServerMessage(line: Data(contentsOf: url)) else { return XCTFail("Missing Task reading") }
        let plan = try result.decode(WorkViewPlan.self)
        let task = try XCTUnwrap(mobileTasks(ProjectedWorkPlan(plan)).first)
        let session = try XCTUnwrap(task.primarySession)
        var selection = "old"
        let operation = owner.createTask(request: "Original words", directory: "/repo") { selection = $0.id }
        await owner.persistPending()
        var observations = server.observations.makeAsyncIterator()
        _ = await observations.next()
        server.publish(try creationFrame(id: operation.id, taskID: task.work.id))
        await waitForCreation(operation) { operation.receipt != nil }
        XCTAssertNil(operation.sessionID)
        selection = "B"
        server.publish(try creationFrame(id: operation.id, delivery: "finished", revision: 2, session: session, taskID: task.work.id))
        await waitForCreation(operation) { operation.sessionID != nil }
        XCTAssertEqual(selection, "B")
        let (attaching, entered) = AsyncStream<Void>.makeStream()
        var release: CheckedContinuation<Void, Never>?
        var attachments: [String] = []
        client.attachForTesting = { id, _ in
            attachments.append(id)
            if attachments.count == 1 {
                entered.yield(())
                await withCheckedContinuation { release = $0 }
            }
            throw RegistryQueryError("Attachment failed")
        }
        let link = SessionLink(client: client, task: task)
        defer { release?.resume(); entered.finish(); link.close() }
        link.fit(columns: 80, rows: 24)
        var pending = attaching.makeAsyncIterator()
        _ = await pending.next()
        XCTAssertEqual(link.state, .opening)
        link.send(Data("unsent input".utf8))
        XCTAssertEqual(link.state, .opening)
        release?.resume(); release = nil
        while link.state == .opening {
            await withCheckedContinuation { continuation in
                withObservationTracking { _ = link.state } onChange: { continuation.resume() }
            }
        }
        if case .disconnected = link.state { } else { XCTFail("Attachment failure must remain recoverable") }
        await link.open()
        XCTAssertEqual(attachments, [session, session])
        XCTAssertEqual(selection, "B")
        XCTAssertTrue(server.submissions.isEmpty)
        let recovery = try await disk.recovery(operation.id)
        XCTAssertEqual(recovery?.resultPresented, false)
    }

    func testConflictingLegacyOutboxRetainsTheCompleteSourceFile() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let disk = ClientWorkStore(directory: root.appendingPathComponent("outbox"), machine: { "machine" }, read: { _ in "null" },
            observe: { _ in ServerWatch(updates: AsyncThrowingStream { _ in }, checkpoint: { _ in }, cancel: {}) }, send: { _ in XCTFail() })
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { owner.shutdown() }
        let id = UUID()
        let original = SubmittedOperation(requestID: id.uuidString.lowercased(), destination: "machine", payload: try JSONEncoder().encode(
            ServedWrite.taskCreate(directory: "/repo", requestID: id.uuidString.lowercased(), title: nil, request: "Different saved words",
                wave: nil, graph: nil, session: nil, run: true)), version: ServerProtocol.revision, submittedAt: Date())
        try await disk.save(original)
        let store = MobileTaskDraftStore(url: root.appendingPathComponent("draft.json"))
        let source: [String: Any] = ["text": "Newer unsent words", "directory": "/repo", "machineID": "machine",
            "submission": ["id": id.uuidString, "machineID": "machine", "machineName": "Original Mac", "directory": "/repo", "request": "Original words"],
            "outcome": ["task_id": "original-task", "state": "unknown"]]
        let bytes = try JSONSerialization.data(withJSONObject: source)
        try bytes.write(to: store.url)
        let editor = MobileTaskCreation(client: EditorClient(interaction: owner, disk: disk), store: store)
        await editor.prepare()
        XCTAssertNotNil(editor.error)
        XCTAssertFalse(editor.saveDraft())
        XCTAssertEqual(try Data(contentsOf: store.url), bytes)
        let pending = try await disk.pending()
        XCTAssertEqual(pending.first?.payload, original.payload)
    }

    func testUnreadableEditorIsNeverOverwritten() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let disk = ClientWorkStore(directory: root.appendingPathComponent("outbox"), machine: { "machine" }, read: { _ in "null" },
            observe: { _ in ServerWatch(updates: AsyncThrowingStream { $0.finish() }, checkpoint: { _ in }, cancel: {}) }, send: { _ in XCTFail() })
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { owner.shutdown() }
        let store = MobileTaskDraftStore(url: root.appendingPathComponent("draft.json"))
        let bad = Data("unreadable draft".utf8)
        try bad.write(to: store.url)
        let model = MobileTaskCreation(client: EditorClient(interaction: owner, disk: disk), store: store)
        await model.prepare()
        model.editText("Do not overwrite")
        XCTAssertFalse(model.saveDraft())
        XCTAssertTrue(model.storageUnavailable)
        XCTAssertEqual(try Data(contentsOf: store.url), bad)
    }
}

@MainActor
private final class EditorClient: MobileTaskCreationClient {
    let interaction: WorkInteraction
    let disk: ClientWorkStore
    var importFailure = false
    var beforeImport: (() throws -> Void)?
    init(interaction: WorkInteraction, disk: ClientWorkStore) { self.interaction = interaction; self.disk = disk }
    func creationContext() async throws -> MobileCreationContext {
        MobileCreationContext(machineID: "machine", name: "Original Mac", repositories: [PortfolioRepo(path: "/repo", error: nil)])
    }
    func draftSubmitted(_ id: String, machine: String?) async throws -> Bool { try await disk.draftSubmitted(id) }
    func importLegacyCreations(_ values: [LegacyOperation]) async throws {
        if importFailure { throw RegistryQueryError("Interrupted import") }
        try beforeImport?()
        try await disk.importLegacy(values)
    }
}

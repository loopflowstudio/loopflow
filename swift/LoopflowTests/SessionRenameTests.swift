import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

/// Named Session drill-down: shared rename readback, retained rejections,
/// and no late publication into another Session or an older read.
@MainActor
@Suite struct SessionRenameTests {
    @Test("Offline rename appears in two windows, survives restart, and rejects older names")
    func offlineRenameUsesOneNameOwner() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        var first = try record("first", title: "Original")
        first.workspace = SessionWorkspace(machineId: "execution-peer", worktree: "/repo", taskId: nil, unavailable: nil)
        let second = try record("second", title: "Other")
        let page = String(decoding: try JSONEncoder().encode(SessionPage(entries: [first, second], next: nil)), as: UTF8.self)
        let query = RegistryQuery { _, _ in page }
        func store() -> ClientWorkStore {
            ClientWorkStore(directory: root, machine: { "machine" }, read: { _ in "null" },
                observe: { _ in ServerWatch(updates: AsyncThrowingStream { _ in }, checkpoint: { _ in }, cancel: {}) },
                send: { _ in Issue.record("No stamped receipt has authorized a send") })
        }
        let disk = store()
        let owner = WorkInteraction(machine: "machine", store: disk)
        let windows = (0..<2).map { _ in WorkModel(query: query, repoPath: "/src/loopflow") }
        for model in windows { model.useInteractionForTesting(owner); await model.refreshSessions() }
        try await owner.receiveNames([first, second], stamp: nameStamp(6))
        windows[0].navigation.selectedSessionId = "first"
        windows[0].beginSessionRename(first)
        windows[0].navigation.renaming?.text = "Launch design"
        await windows[0].commitSessionRename()
        #expect(windows[0].navigation.renaming == nil)
        for model in windows { #expect(title(model, "first") == "Launch design") }
        windows[0].navigation.selectedSessionId = "second"
        windows[0].beginSessionRename(second)
        windows[0].navigation.renaming?.text = "Unsent edit"
        await owner.persistPending()
        let saved = try #require(try await disk.pending().first)
        owner.shutdown()
        let reopened = store()
        let restored = WorkInteraction(machine: "machine", store: reopened)
        defer { restored.shutdown() }
        restored.start()
        await restored.refreshFromStore()
        for model in windows { model.useInteractionForTesting(restored) }
        #expect(title(windows[0], "first") == "Launch design")
        let database = try ClientWorkDatabase(directory: root)
        #expect(try database.observeEpoch("epoch", id: saved.id))
        try database.receive(nameReceipt(saved.id, title: "Launch design"), epoch: "epoch", id: saved.id)
        try database.retireIfConverged(saved.id)
        await restored.refreshFromStore()
        #expect(try await reopened.pending().isEmpty)
        try await restored.receiveNames([first], stamp: nameStamp(5, query: "late-other-window"))
        for model in windows {
            await model.refreshSessions() // an unstamped old page cannot replace a stamped name
            #expect(title(model, "first") == "Launch design")
            #expect(title(model, "second") == "Other")
        }
        #expect(windows[0].navigation.selectedSessionId == "second")
        #expect(windows[0].navigation.renaming?.text == "Unsent edit")
    }

    @Test("Refusal restores only that name, and a later rename waits for its earlier outcome")
    func refusalAndRenameOrdering() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let disk = ClientWorkStore(directory: root, machine: { "machine" }, read: { _ in "null" },
            observe: { _ in ServerWatch(updates: AsyncThrowingStream { _ in }, checkpoint: { _ in }, cancel: {}) }, send: { _ in })
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { owner.shutdown() }
        let first = try record("first", title: "Original")
        let other = try record("second", title: "Other")
        try await owner.receiveNames([first, other], stamp: nameStamp(6))
        let refused = owner.renameSession(id: first.id, name: "Refused", directory: "/repo")
        let later = owner.renameSession(id: first.id, name: "Later", directory: "/repo")
        owner.renameSession(id: other.id, name: "Other edit", directory: "/repo")
        await owner.persistPending()
        let database = try ClientWorkDatabase(directory: root)
        let laterRecord = try #require(database.state(later)?.record)
        #expect(try database.earlierRenamePending(laterRecord))
        #expect(try database.observeEpoch("epoch", id: refused))
        try database.receive(nameReceipt(refused, title: "Refused", refused: true), epoch: "epoch", id: refused)
        #expect(try !database.earlierRenamePending(laterRecord))
        await owner.refreshFromStore()
        #expect(owner.named(first).title == "Later")
        #expect(owner.named(other).title == "Other edit")
        #expect(try database.observeEpoch("epoch", id: later))
        try database.receive(nameReceipt(later, title: "Later", refused: true), epoch: "epoch", id: later)
        await owner.refreshFromStore()
        #expect(owner.named(first).title == "Original")
        #expect(owner.named(other).title == "Other edit")
    }

    @Test("A newer edit replaces a rename that never saved")
    func failedSaveCannotRetryBehindANewerName() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let fault = IntentDiskFault()
        let disk = ClientWorkStore(directory: root, prepareDirectory: { _ in try fault.check() }, machine: { "machine" },
            read: { _ in "null" }, observe: { _ in
                ServerWatch(updates: AsyncThrowingStream { _ in }, checkpoint: { _ in }, cancel: {})
            }, send: { _ in Issue.record("No send without a stamped reading") })
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { owner.shutdown() }
        let session = try record("first", title: "Original")
        let failed = owner.renameSession(id: "first", name: "Never saved", directory: "/repo")
        await owner.persistPending()
        #expect(owner.named(session).title == "Original")
        #expect(owner.listedOperations.first?.presentation().actions == [.tryAgain])
        fault.recover()
        let newer = owner.renameSession(id: "first", name: "New name", directory: "/repo")
        await owner.persistPending()
        owner.retry(failed)
        #expect(owner.named(session).title == "New name")
        #expect(try await disk.pending().map(\.id) == [newer])
    }

    @Test("Reconnect recovers the first rename by ID before delivering the next name once")
    func reconnectKeepsRenameOrderAndOriginalIDs() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let machine = RenameMachine()
        func store() -> ClientWorkStore {
            ClientWorkStore(directory: root, machine: { "machine" }, read: { _ in "null" },
                observe: { machine.observe($0) }, send: { try machine.send($0) })
        }
        let disk = store()
        let owner = WorkInteraction(machine: "machine", store: disk)
        let first = owner.renameSession(id: "first", name: "First name", directory: "/repo")
        let second = owner.renameSession(id: "first", name: "Later name", directory: "/repo")
        await owner.persistPending()
        var sends = machine.sends.makeAsyncIterator()
        #expect(await sends.next() == first)
        #expect(machine.requestIDs == [first], "The next name waits for the first domain outcome")
        owner.shutdown()
        for await _ in await disk.changes() {
            if await !disk.isRecovering(first), await !disk.isRecovering(second) { break }
        }
        // The first command committed, but both its answer and event were lost.
        machine.publish(try nameReceipt(first, title: "First name"))
        let reopened = store()
        let restored = WorkInteraction(machine: "machine", store: reopened)
        defer { restored.shutdown() }
        restored.start()
        #expect(await sends.next() == second)
        #expect(machine.requestIDs == [first, second])
        machine.publish(try nameReceipt(second, title: "Later name"))
        for await _ in await reopened.changes() {
            if try await reopened.pending().isEmpty { break }
        }
        await restored.refreshFromStore()
        #expect(restored.sessionTitle(id: "first", fallback: "Original") == "Later name")
        #expect(machine.requestIDs == [first, second], "Recovery never renames again under a fresh ID")
    }

    @Test("A committed name survives a later output failure")
    func committedNameIsNeverRolledBackByOutputFailure() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let disk = ClientWorkStore(directory: root, machine: { "machine" }, read: { _ in "null" },
            observe: { _ in ServerWatch(updates: AsyncThrowingStream { _ in }, checkpoint: { _ in }, cancel: {}) }, send: { _ in })
        let owner = WorkInteraction(machine: "machine", store: disk)
        defer { owner.shutdown() }
        let id = owner.renameSession(id: "first", name: " Requested ", directory: "/repo")
        await owner.persistPending()
        let database = try ClientWorkDatabase(directory: root)
        #expect(try database.observeEpoch("epoch", id: id))
        var body = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(nameReceipt(id, title: "Requested"))) as? [String: Any])
        body["error"] = "Output lost after the name changed"
        try database.receive(JSONDecoder().decode(OperationReceipt.self, from: JSONSerialization.data(withJSONObject: body)), epoch: "epoch", id: id)
        await owner.refreshFromStore()
        #expect(owner.sessionTitle(id: "first", fallback: "Original") == "Requested")
        #expect(owner.listedOperations.first?.presentation().actions.contains(.inspect) == true)
    }

    @Test("The breadcrumb drills Wave → Task → exact Session, with siblings as the final choice")
    func breadcrumbNamesTheExactSession() throws {
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(roadmapJSON().utf8))
        let task = WorkReference.task(id: "issue-review")
        let one = WorkProjection(roadmaps: roadmap.waves, inventory: roadmap.tasks.items, sessions: [try record("first", title: "review-design", work: task)])
        let single = try #require(one.breadcrumb(selection: nil, sessionId: "first"))
        #expect(single.wave?.roadmap.wave.name == "product")
        #expect(single.task?.task.task.identifier == "W2-131")
        #expect(single.session?.id == "first")
        #expect(single.siblings.map(\.id) == ["first"])

        let many = WorkProjection(roadmaps: roadmap.waves, inventory: roadmap.tasks.items, sessions: [
            try record("first", title: "Launch design", work: task),
            try record("second", title: "Verification cases", work: task),
            try record("loose", title: "lyric-cadenza", work: nil),
        ])
        let multiple = try #require(many.breadcrumb(selection: .task(id: "issue-review"), sessionId: "second"))
        #expect(multiple.session?.title == "Verification cases")
        #expect(multiple.siblings.map(\.id) == ["first", "second"])
        let taskOnly = try #require(many.breadcrumb(selection: .task(id: "issue-review"), sessionId: nil))
        #expect(taskOnly.session == nil)
        #expect(taskOnly.task?.task.task.identifier == "W2-131")
        let unmatched = try #require(many.breadcrumb(selection: nil, sessionId: "loose"))
        #expect(unmatched.wave == nil && unmatched.task == nil)
    }

    private func title(_ model: WorkModel, _ id: String) -> String? {
        model.sessions.value?.first { $0.id == id }?.title
    }
}

func renameFixtureRecord(
    _ id: String, title: String, source: String = "generated", work: WorkReference? = nil
) throws -> SessionRecord {
    let workJSON = String(decoding: try JSONEncoder().encode(work), as: UTF8.self)
    let taskIds = work.flatMap { $0.kind == .task ? [$0.id] : nil } ?? []
    let taskIdsJSON = String(decoding: try JSONEncoder().encode(taskIds), as: UTF8.self)
    return try JSONDecoder().decode(SessionRecord.self, from: Data("""
    {"id":"\(id)","run_id":"run_\(id)", "interactive": true,"work":\(workJSON),"title":"\(title)",
     "detail":"codex","cwd":"/src/loopflow","state":"active","wave_id":null,"work_path":null,
     "actions":\(sessionActionFixtureJSON(state: "active")),
     "ready_summary":null,"blocked": false, "title_source":"\(source)","task_primary":false, "graph_membership":{"kind":"independent"},
     "task_ids": \(taskIdsJSON), "agent_process_id": "44444444-4444-4444-8444-444444444444", "turn":null,"raw":false,"terminal_ids":[],"open_argv":["lf","session","resume","\(id)"]}
    """.utf8))
}

private func record(_ id: String, title: String, work: WorkReference? = nil) throws -> SessionRecord {
    try renameFixtureRecord(id, title: title, work: work)
}

private func roadmapJSON() throws -> String {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        .deletingLastPathComponent().deletingLastPathComponent()
    return try String(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json"), encoding: .utf8)
}

private func nameStamp(_ revision: Int64, query: String = "sessions") throws -> ReadingStamp {
    try JSONDecoder().decode(ReadingStamp.self, from: JSONSerialization.data(withJSONObject: [
        "machine": "machine", "epoch": "epoch", "schema": ServerProtocol.revision, "query": query,
        "complete": true, "observations": NSNull(), "revisions": ["planning": 1, "sessions": revision, "graphs": 1, "processes": 1, "usage": 1]]))
}

private func nameReceipt(_ id: String, title: String, refused: Bool = false) throws -> OperationReceipt {
    let commit: [String: Any] = ["stage": "session.renamed", "resolution": refused ? "definitely_refused" : "committed",
        "resources": ["first"], "fence": ["machine": "machine", "epoch": "epoch", "schema": ServerProtocol.revision,
            "revisions": ["sessions": 7]], "result": ["id": "first", "title": title, "title_source": "human"],
        "planning_effect": "none", "planning_changes": []]
    return try JSONDecoder().decode(OperationReceipt.self, from: JSONSerialization.data(withJSONObject: [
        "request_id": id, "kind": "session.rename", "fingerprint": NSNull(), "submitted_fingerprint": "hash",
        "validation": "bound", "delivery": "finished", "revision": 2, "result": NSNull(),
        "error": refused ? "Name refused" : NSNull(), "commits": [commit], "planning": NSNull(), "created_at": 1, "updated_at": 2]))
}

private final class RenameMachine: @unchecked Sendable {
    private let lock = NSLock()
    private var receipts: [String: OperationReceipt] = [:]
    private var watches: [UUID: (String, AsyncThrowingStream<SubscriptionReading<OperationReceipt>, any Error>.Continuation)] = [:]
    private var submitted: [String] = []
    let sends: AsyncStream<String>
    private let sending: AsyncStream<String>.Continuation
    init() { (sends, sending) = AsyncStream.makeStream() }
    var requestIDs: [String] { lock.withLock { submitted } }

    func observe(_ id: String) -> ClientWorkStore.ReceiptWatch {
        let key = UUID()
        let (stream, continuation) = AsyncThrowingStream<SubscriptionReading<OperationReceipt>, any Error>.makeStream()
        lock.withLock {
            watches[key] = (id, continuation)
            continuation.yield(frame(receipts[id]))
        }
        return ServerWatch(updates: stream, checkpoint: { _ in }, cancel: { [self] in
            lock.withLock { watches.removeValue(forKey: key)?.1.finish() }
        })
    }

    func send(_ data: Data) throws {
        let command = try JSONDecoder().decode(JSONValue.self, from: data)
        guard case .string(let id) = command["request_id"] else { throw RegistryQueryError("Missing request ID") }
        lock.withLock { submitted.append(id) }
        sending.yield(id)
        throw ServerConnectionUnavailable(RegistryQueryError("Answer lost"))
    }

    func publish(_ receipt: OperationReceipt) {
        lock.withLock {
            receipts[receipt.requestID] = receipt
            for (id, continuation) in watches.values where id == receipt.requestID { continuation.yield(frame(receipt)) }
        }
    }

    private func frame(_ receipt: OperationReceipt?) -> SubscriptionReading<OperationReceipt> {
        try! JSONDecoder().decode(SubscriptionReading<OperationReceipt>.self, from: JSONSerialization.data(withJSONObject: [
            "answers": NSNull(), "unavailable": NSNull(), "stamp": JSONSerialization.jsonObject(with: JSONEncoder().encode(nameStamp(12))),
            "body": receipt.map { try! JSONSerialization.jsonObject(with: JSONEncoder().encode($0)) } ?? NSNull()]))
    }
}

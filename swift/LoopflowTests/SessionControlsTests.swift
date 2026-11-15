import Foundation
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@MainActor
@Suite struct SessionControlsTests {
    @Test("Binding preview matches Rust and requires every identity field")
    func bindingPreviewFixture() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let data = try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/session_binding_preview.json"))
        let preview = try JSONDecoder().decode(SessionBindingPreview.self, from: data)
        #expect(preview.sessionId == "conversation-unchanged")
        #expect(preview.identifier == "INF-123")
        let original = try #require(JSONSerialization.jsonObject(with: data) as? [String: String])
        #expect(try JSONSerialization.jsonObject(with: JSONEncoder().encode(preview)) as? [String: String] == original)
        for key in original.keys {
            var missing = original
            missing.removeValue(forKey: key)
            #expect(throws: DecodingError.self) {
                try JSONDecoder().decode(SessionBindingPreview.self, from: JSONSerialization.data(withJSONObject: missing))
            }
        }
    }

    @Test("Preview is inert; confirmation uses exact Task and fences an older poll")
    func bindAfterPreview() async throws {
        let source = try ControlsSource()
        let model = WorkModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        await model.refreshSessions()
        model.navigation.selectedSessionId = "interactive"
        model.beginSessionBinding(try #require(model.sessions.value?.first { $0.id == "interactive" }))
        model.navigation.binding?.selector = "INF-123"
        await model.previewSessionBinding()
        #expect(model.sessions.value?.first { $0.id == "interactive" }?.work == nil)
        #expect(model.navigation.binding?.preview?.taskId == "task-exact")
        let sheet = SessionActionSheet(model: model)
        _ = try sheet.inspect().find(button: "Bind to Task")
        #expect(throws: (any Error).self) { try sheet.inspect().find(button: "Assign to Task") }
        #expect(throws: (any Error).self) { try sheet.inspect().find(button: "Make primary Task conversation") }
        #expect(await source.bindCount == 0)
        await source.holdList()
        let poll = Task { await model.refreshSessions() }
        await source.waitForList()
        await model.commitSessionBinding()
        await source.releaseList()
        await poll.value
        #expect(model.navigation.binding == nil)
        #expect(model.navigation.selectedSessionId == "interactive")
        let crumb = try #require(model.projection.breadcrumb(selection: model.selection, sessionId: "interactive"))
        #expect(crumb.task == nil) // No roadmap entry is needed.
        #expect(crumb.taskWork == .task(id: "task-exact"))
        #expect(crumb.waveWork == .wave(id: "wave-exact"))
        #expect(await source.bindCount == 1)
    }

    @Test("A refused bind keeps its exact confirmation for safe same-target retry")
    func bindRefusalAndRetry() async throws {
        let source = try ControlsSource()
        let model = WorkModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        await model.refreshSessions()
        model.navigation.selectedSessionId = "interactive"
        model.beginSessionBinding(try #require(model.sessions.value?.first { $0.id == "interactive" }))
        model.navigation.binding?.selector = "INF-123"
        await model.previewSessionBinding()
        await source.rejectBind()
        await model.commitSessionBinding()
        #expect(model.navigation.binding?.preview?.taskId == "task-exact")
        #expect(model.navigation.binding?.error == "Session changed before binding")
        #expect(model.sessions.value?.first { $0.id == "interactive" }?.work == nil)
        await model.commitSessionBinding()
        #expect(model.navigation.binding == nil)
        #expect(model.sessions.value?.first { $0.id == "interactive" }?.work == .task(id: "task-exact"))
    }
    @Test("Creation and binding keep the selected conversation and change its Work without launching")
    func createOrBindConversation() async throws {
        for existing in [false, true] {
            let source = try ControlsSource()
            let model = WorkModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
            await model.refreshSessions()
            model.navigation.selectedSessionId = "interactive"
            let session = try #require(model.sessions.value?.first { $0.id == "interactive" })
            model.beginSessionBinding(session)
            if existing {
                model.navigation.binding?.selector = "INF-123"
                await model.previewSessionBinding()
            }
            if existing { await model.commitSessionBinding() }
            else { await model.createTaskFromSession() }
            #expect(model.navigation.binding == nil)
            #expect(model.navigation.selectedSessionId == session.id)
            #expect(model.selection == .task(id: "task-exact"))
            let promoted = try #require(model.sessions.value?.first { $0.id == session.id })
            #expect(promoted.work == .task(id: "task-exact"))
            #expect(promoted.cwd == (existing ? "/src/delivery" : session.cwd))
            #expect(promoted.agentProcessId == session.agentProcessId)
            #expect(promoted.taskPrimary)
            #expect(await source.bindCount == (existing ? 1 : 0))
            if !existing {
                let requestID = try #require(await source.creationID)
                #expect(CreationRecovery.shared.requests(for: .task("task-exact")).contains(requestID))
                await CreationRecovery.shared.presented([requestID])
            }
        }
    }

    @Test("An assigned non-primary Session offers primary selection through binding")
    func bindAssignedConversation() async throws {
        let source = try ControlsSource(assigned: true)
        let model = WorkModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        await model.refreshSessions()
        model.navigation.selectedSessionId = "interactive"
        let session = try #require(model.sessions.value?.first)
        #expect(model.searchDestinations("Primary").map(\.id) == [.bind(session.id)])
        #expect(model.searchDestinations("Bind").isEmpty)
        model.beginSessionBinding(session)
        let sheet = SessionActionSheet(model: model)
        _ = try sheet.inspect().find(button: "Make primary Task conversation")
        #expect(throws: (any Error).self) { try sheet.inspect().find(button: "Create Task from this conversation") }
        #expect(throws: (any Error).self) { try sheet.inspect().find(button: "Assign to Task") }
        await model.commitSessionBinding()
        #expect(await source.bindCount == 1)
        #expect(model.navigation.selectedSessionId == session.id)
        #expect(model.sessions.value?.first?.agentProcessId == session.agentProcessId)
        #expect(model.sessions.value?.first?.cwd == "/src/delivery")
        #expect(model.sessions.value?.first?.taskPrimary == true)
        #expect(model.searchDestinations("Primary").isEmpty)
    }

    @Test("Task actions offer primary selection for an already assigned Session")
    func assignedConversationTaskAction() async throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from:
            Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")))
        let task = try #require(roadmap.tasks.items.first)
        let record = try renameFixtureRecord("assigned", title: "Research", work: .task(id: task.runtime?.workId ?? task.id))
        let entries = String(decoding: try JSONEncoder().encode([record]), as: UTF8.self)
        let model = WorkModel(query: RegistryQuery { args, _ in
            if args.prefix(2) == ["session", "list"] { return #"{"entries":\#(entries),"next":null}"# }
            throw RegistryQueryError("Reading actions must not change work")
        }, repoPath: task.task.repo)
        model.applyFixture(roadmap: .available(roadmap), waves: .available([]), workActivity: .loading, repos: [])
        await model.refreshSessions()
        let actions = TaskActionItems(model: model, task: task, wave: nil)
        try actions.inspect().find(button: "Make Research primary…").tap()
        #expect(model.navigation.binding?.sessionId == record.id)
        #expect(model.navigation.binding?.assignedTaskId == record.work?.id)
        #expect(model.navigation.binding?.preview == nil)
    }

}

private actor ControlsSource {
    private var record: [String: Any]
    private var hold = false
    private var reject = false
    private var pending: CheckedContinuation<Void, Never>?
    private var waiter: CheckedContinuation<Void, Never>?
    private(set) var bindCount = 0
    private(set) var creationID: String?

    init(assigned: Bool = false) throws {
        let session = try renameFixtureRecord("interactive", title: "Conversation")
        record = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(session)) as? [String: Any])
        if assigned { record["work"] = ["kind": "task", "id": "task-exact"] }
    }

    func holdList() { hold = true }
    func rejectBind() { reject = true }
    func waitForList() async {
        if pending != nil { return }
        await withCheckedContinuation { waiter = $0 }
    }
    func releaseList() { pending?.resume(); pending = nil }

    func read(_ args: [String]) async throws -> String {
        if args.prefix(2) == ["session", "list"] {
            let snapshot = try JSONSerialization.data(withJSONObject: ["entries": [record], "next": NSNull()])
            if hold {
                hold = false
                await withCheckedContinuation { pending = $0; waiter?.resume(); waiter = nil }
            }
            return String(decoding: snapshot, as: UTF8.self)
        }
        if args.starts(with: ["task", "create"]) {
            creationID = args[3]
            #expect(args.contains("--session"))
            #expect(!args.contains("--run"))
            record["work"] = ["kind": "task", "id": "task-exact"]
            record["task_primary"] = true
            return #"{"task":{"id":"task-exact","identifier":"INF-123","title":"Conversation"},"graph":null,"session_id":null,"milestone":"filed"}"#
        }
        if args.contains("--dry-run") {
            return #"{"session_id":"interactive","task_id":"task-exact","identifier":"INF-123","title":"Completed off-roadmap Task"}"#
        }
        #expect(args == ["session", "bind", "--json", "--task", "task-exact", "--", "interactive"])
        bindCount += 1
        if reject { reject = false; throw RegistryQueryError("Session changed before binding") }
        record["work"] = ["kind": "task", "id": "task-exact"]
        record["task_primary"] = true
        record["cwd"] = "/src/delivery"
        record["wave_id"] = "wave-exact"
        return String(decoding: try JSONSerialization.data(withJSONObject: record), as: UTF8.self)
    }
}

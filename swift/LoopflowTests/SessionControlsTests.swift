import Foundation
import Testing
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

    @Test("Headless visibility does not change inventory, selected identity or rename draft")
    func visibilityRetainsInventory() async throws {
        let source = try ControlsSource()
        let model = PodiumModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        model.navigation.showsHeadlessSessions = true
        await model.refreshSessions()
        #expect(model.sessions.value?.count == 2)
        model.navigation.showsHeadlessSessions = false
        #expect(model.visibleWorkspace.orphanSessions(search: "").map(\.id) == ["interactive"])
        model.navigation.showsHeadlessSessions = true
        #expect(Set(model.visibleWorkspace.orphanSessions(search: "").map(\.id)) == ["interactive", "headless"])
        model.navigation.selectedSessionId = "headless"
        model.beginSessionRename(try #require(model.sessions.value?.first { $0.id == "headless" }))
        model.navigation.renaming?.text = "Retained draft"
        model.navigation.showsHeadlessSessions = false
        await model.refreshSessions()
        #expect(model.navigation.selectedSessionId == "headless")
        #expect(model.navigation.renaming?.text == "Retained draft")
        #expect(model.sessions.value?.map(\.id) == ["interactive"])
        #expect(model.visibleWorkspace.orphanSessions(search: "").map(\.id) == ["interactive"])
    }


    @Test("Preview is inert; confirmation uses exact Task and fences an older poll")
    func bindAfterPreview() async throws {
        let source = try ControlsSource()
        let model = PodiumModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        model.navigation.showsHeadlessSessions = true
        await model.refreshSessions()
        model.navigation.selectedSessionId = "headless"
        model.beginSessionBinding(try #require(model.sessions.value?.first { $0.id == "headless" }))
        model.navigation.binding?.selector = "INF-123"
        await model.previewSessionBinding()
        #expect(model.sessions.value?.first { $0.id == "headless" }?.work == nil)
        #expect(model.navigation.binding?.preview?.taskId == "task-exact")
        #expect(await source.bindCount == 0)
        await source.holdList()
        let poll = Task { await model.refreshSessions() }
        await source.waitForList()
        await model.commitSessionBinding()
        await source.releaseList()
        await poll.value
        #expect(model.navigation.binding == nil)
        #expect(model.navigation.selectedSessionId == "headless")
        let crumb = try #require(model.workspace.breadcrumb(selection: model.selection, sessionId: "headless"))
        #expect(crumb.task == nil) // No roadmap entry is needed.
        #expect(crumb.taskWork == .task(id: "task-exact"))
        #expect(crumb.waveWork == .wave(id: "wave-exact"))
        #expect(await source.bindCount == 1)
    }

    @Test("A refused bind keeps its exact confirmation for safe same-target retry")
    func bindRefusalAndRetry() async throws {
        let source = try ControlsSource()
        let model = PodiumModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        model.navigation.showsHeadlessSessions = true
        await model.refreshSessions()
        model.navigation.selectedSessionId = "headless"
        model.beginSessionBinding(try #require(model.sessions.value?.first { $0.id == "headless" }))
        model.navigation.binding?.selector = "INF-123"
        await model.previewSessionBinding()
        await source.rejectBind()
        await model.commitSessionBinding()
        #expect(model.navigation.binding?.preview?.taskId == "task-exact")
        #expect(model.navigation.binding?.error == "Session changed before binding")
        #expect(model.sessions.value?.first { $0.id == "headless" }?.work == nil)
        await model.commitSessionBinding()
        #expect(model.navigation.binding == nil)
        #expect(model.sessions.value?.first { $0.id == "headless" }?.work == .task(id: "task-exact"))
    }
}

private actor ControlsSource {
    private var rows: [[String: Any]]
    private var hold = false
    private var reject = false
    private var pending: CheckedContinuation<Void, Never>?
    private var waiter: CheckedContinuation<Void, Never>?
    private(set) var bindCount = 0

    init() throws {
        rows = try ["interactive", "headless"].map { id in
            let record = try renameFixtureRecord(id, title: id)
            var row = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(record)) as? [String: Any])
            row["interactive"] = id == "interactive"
            return row
        }
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
            let visible = args.contains("all") ? rows : rows.filter { $0["interactive"] as? Bool == true }
            let snapshot = try JSONSerialization.data(withJSONObject: ["entries": visible, "next": NSNull()])
            if hold {
                hold = false
                await withCheckedContinuation { pending = $0; waiter?.resume(); waiter = nil }
            }
            return String(decoding: snapshot, as: UTF8.self)
        }
        if args.contains("--dry-run") {
            return #"{"session_id":"headless","task_id":"task-exact","identifier":"INF-123","title":"Completed off-roadmap Task"}"#
        }
        #expect(args == ["session", "bind", "--json", "--task", "task-exact", "--", "headless"])
        bindCount += 1
        if reject { reject = false; throw RegistryQueryError("Session changed before binding") }
        rows[1]["work"] = ["kind": "task", "id": "task-exact"]
        rows[1]["wave_id"] = "wave-exact"
        return String(decoding: try JSONSerialization.data(withJSONObject: rows[1]), as: UTF8.self)
    }
}

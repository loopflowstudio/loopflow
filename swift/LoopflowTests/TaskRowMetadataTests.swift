#if os(macOS)
import Foundation
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Sidebar Task metadata")
@MainActor
struct TaskRowMetadataTests {
    @Test("Validated reports win over runtime inference", arguments: [ProgramStatusState.idle, .working, .done, .blocked, .error])
    func reportedStatus(state: ProgramStatusState) throws {
        let report = ProgramStatusReport(state: state, id: "worker", kind: nil, progress: nil,
                                         app: "cargo", title: nil, msg: "**literal**\u{202E}")
        let reporting = try session("reporter", status: ProgramStatusRecords(seen: true, records: [report]))
        let waiting = try session("waiting", attention: .waiting)
        let status = SessionProgramStatus(sessions: [waiting, reporting], unavailable: nil)
        #expect(status.label == state.rawValue.capitalized)
        #expect(status.help.contains("reporter"))
        #expect(status.help.contains("**literal**"))
        #expect(!status.help.contains("\u{202E}"))
        // A program result never removes unresolved Task work from the outline.
        let snapshot = try roadmap()
        let rows = WorkProjection(roadmaps: snapshot.waves, sessions: [reporting])
            .outline(collapsed: [], search: "", planningReadable: true)
        #expect(rows.contains { $0.task?.id == snapshot.waves[0].tasks.items[1].id })
    }

    @Test("Different terminals retain independent report ids and source attribution")
    func reportSources() throws {
        let working = ProgramStatusReport(state: .working, id: nil, kind: nil, progress: nil, app: nil, title: nil, msg: nil)
        let blocked = ProgramStatusReport(state: .blocked, id: nil, kind: .auth, progress: nil, app: nil, title: nil, msg: "Sign in")
        let sessions = [try session("a", status: .init(seen: true, records: [working])),
                        try session("b", status: .init(seen: true, records: [blocked]))]
        let status = SessionProgramStatus(sessions: sessions, unavailable: nil)
        #expect(status.label == "Blocked · Auth")
        #expect(status.help.contains("(b)"))
    }

    @Test("Live clients and unknown readings omit activity; shared Waiting remains visible")
    func inference() throws {
        let active = try session("active")
        let waiting = try session("waiting", attention: .waiting)
        #expect(SessionProgramStatus(sessions: [active], unavailable: nil).label == nil)
        #expect(SessionProgramStatus(sessions: [active, waiting], unavailable: nil).label == "Waiting · inferred")
        let unknown = try session("unknown", state: .unknown)
        let closed = try session("closed", state: .closed, attention: .waiting)
        #expect(SessionProgramStatus(sessions: [unknown, closed], unavailable: nil).label == nil)
        #expect(SessionProgramStatus(sessions: [active], unavailable: "Offline").label == nil)
    }

    @Test("Cleared reporter records do not revive inferred activity or Waiting")
    func clearDoesNotInfer() throws {
        let cleared = try session("cleared", attention: .waiting, status: .init(seen: true, records: []))
        #expect(SessionProgramStatus(sessions: [cleared], unavailable: nil).label == nil)
    }

    @Test("Saved readings cannot resurrect program reports or Waiting")
    func savedStatusIsUnknown() throws {
        let report = ProgramStatusReport(state: .blocked, id: nil, kind: .question, progress: nil,
                                         app: nil, title: nil, msg: "Proceed?")
        let record = try session("saved", attention: .waiting, status: .init(seen: true, records: [report]))
        let page = SessionPage(entries: [record], next: nil)
        let wire = String(decoding: try JSONEncoder().encode(page), as: UTF8.self)
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        // Simulate a file saved by a previous app, before status was stripped.
        var snapshot = WorkSnapshot()
        snapshot.repositories["/src/loopflow"] = .init(sessionPages: [wire], selection: nil, savedAt: Date())
        let saved = try JSONSerialization.jsonObject(with: JSONEncoder().encode(snapshot))
        try JSONSerialization.data(withJSONObject: ["version": WorkCache.version, "snapshot": saved])
            .write(to: directory.appendingPathComponent("workspace.json"))
        let model = WorkModel(query: RegistryQuery { _, _ in throw RegistryQueryError("No fresh read") },
                              repoPath: "/src/loopflow", cache: WorkCache(directory: directory))
        let restored = try #require(model.sessions.value)
        #expect(restored.map(\.id) == [record.id])
        #expect(SessionProgramStatus(sessions: restored, unavailable: nil).label == nil)
    }

    @Test("Unknown and live-client-only rows omit status; read failures stay inspectable")
    func quietStatus() throws {
        let task = try #require(roadmap().waves.first?.tasks.items[1])
        for records in [[], [try session("live")]] {
            let status = SessionProgramStatus(sessions: records, unavailable: nil)
            #expect(status.label == nil)
            #expect(status.tone == nil)
            let view = TaskRowMetadata(task: task, status: status, localMachineId: nil)
            #expect(throws: (any Error).self) {
                try view.inspect().find(viewWithAccessibilityIdentifier: "work-program-status-\(task.id)")
            }
        }
        let unavailable = TaskRowMetadata(task: task, status: .unavailable("Offline"), localMachineId: nil)
        let icon = try unavailable.inspect().find(viewWithAccessibilityIdentifier: "work-program-status-unavailable-\(task.id)")
        #expect(try icon.accessibilityLabel().string().contains("Offline"))
    }

    @Test("Selection preserves reported failure and shared Waiting semantics")
    func selectedStatus() throws {
        let task = try #require(roadmap().waves.first?.tasks.items[1])
        let error = ProgramStatusReport(state: .error, id: nil, kind: nil, progress: nil,
                                       app: nil, title: nil, msg: "Build failed")
        let reported = SessionProgramStatus(sessions: [try session("failed", status: .init(seen: true, records: [error]))], unavailable: nil)
        #expect(reported.tone == .failure)
        let selected = TaskRowMetadata(task: task, status: reported, localMachineId: nil).workSelection(true)
        #expect(try selected.inspect().find(text: "Error").string() == "Error")
        let waiting = SessionProgramStatus(sessions: [try session("waiting", attention: .waiting)], unavailable: nil)
        #expect(waiting.tone == .waiting)
        #expect(waiting.label == "Waiting · inferred")
        #expect(ProgramStatusState.working.workTone == .running)
        #expect(ProgramStatusState.blocked.workTone == .waiting)
        #expect(ProgramStatusState.done.workTone == .done)
    }

    @Test("Only a known remote checkout gets a Machine name")
    func machineOwnership() throws {
        let task = try #require(roadmap().waves.first?.tasks.items[1])
        let workspace = try #require(task.reference.workspace)
        #expect(workspace.sidebarMachine(localMachineId: workspace.machineId) == nil)
        #expect(workspace.sidebarMachine(localMachineId: "other-machine") == "build-server")
        #expect(workspace.sidebarMachine(localMachineId: nil) == "Machine unknown")
        let unknown = try JSONDecoder().decode(TaskWorktreeSnapshot.self, from: Data("""
        {"machine_id":null,"machine_label":"must-not-guess","slug":"task","branch":null,
         "base_commit":"base","worktree":"/task","local_exists":null}
        """.utf8))
        #expect(unknown.sidebarMachine(localMachineId: "local") == "Machine unknown")
        let unnamed = try JSONDecoder().decode(TaskWorktreeSnapshot.self, from: Data("""
        {"machine_id":"remote","machine_label":null,"slug":"task","branch":null,
         "base_commit":"base","worktree":"/task","local_exists":null}
        """.utf8))
        #expect(unnamed.sidebarMachine(localMachineId: "local") == "Remote · name unavailable")
    }

    @Test("Stored links are separate from retained conversation navigation")
    func linksPreserveWorkspace() throws {
        let snapshot = try roadmap()
        let task = try #require(snapshot.waves.first?.tasks.items[1])
        let query = RegistryQuery { _, _ in throw RegistryQueryError("No reads during link clicks") }
        let model = WorkModel(query: query, repoPath: "/src/loopflow")
        model.applyFixture(roadmap: .available(snapshot), waves: .available([]), workActivity: .loading, repos: [], fixed: true)
        let registry = SessionsWorkspaceRegistry(localMachineId: "local")
        let content = SessionsContentView(model: model, repoPath: "/src/loopflow", workspaces: registry,
                                         machineId: "local", repository: "repo", query: query)
        _ = content.body
        model.select(.task(id: task.id))
        let key = try #require(registry.selectedWork)
        let workspace = try #require(registry.retained(key))
        let record = try session("retained")
        registry.revealSession(record, in: workspace)
        let files = workspace.files(taskId: task.id, issue: task.task.identifier, cwd: record.cwd)
        let document = files.document("draft.md")
        document.editor.string = "unfinished"
        let layout = workspace.multiplexer.layout
        let selected = workspace.presentation.selectedSessionId
        var conversationOpened = false
        let view = WorkNavigator(model: model, onOpenSession: { _ in conversationOpened = true },
                                 onOpenTask: { _ in conversationOpened = true })

        let inspected = try view.inspect()
        var destinations: [URL] = []
        for kind in ["task", "pr"] {
            let link = try inspected.find(ViewType.Link.self, where: {
                try $0.accessibilityIdentifier() == "work-\(kind)-link-\(task.id)"
            })
            destinations.append(try link.url())
            #expect(throws: (any Error).self) { try link.find(ViewType.Button.self, relation: .parent) }
        }
        #expect(destinations == [try #require(task.reference.issueUrl), try #require(task.pr?.publication?.github?.url)])
        #expect(!conversationOpened)
        #expect(model.selection == .task(id: task.id))
        #expect(registry.selectedWork == key)
        #expect(workspace.presentation.selectedSessionId == selected)
        #expect(workspace.multiplexer.layout == layout)
        #expect(files.document("draft.md") === document)
        #expect(document.editor.string == "unfinished")
        try inspected.find(ViewType.Button.self, where: {
            try $0.accessibilityIdentifier() == "work-task-\(task.id)"
        }).tap()
        #expect(conversationOpened)
    }

    private func roadmap() throws -> RoadmapSnapshot {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        return try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")))
    }

    private func session(_ id: String, state: SessionState = .active, attention: SessionAttention? = nil,
                         status: ProgramStatusRecords? = nil) throws -> SessionRecord {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        var value = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/session.json"))) as? [String: Any])
        value["id"] = id
        value["title"] = id
        value["state"] = state.rawValue
        value["task_ids"] = ["ts_review00000000000000000000000000"]
        value["attention"] = attention?.rawValue ?? NSNull() as Any
        value["program_status"] = try status.map { try JSONSerialization.jsonObject(with: JSONEncoder().encode($0)) } ?? NSNull()
        return try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: value))
    }
}
#endif

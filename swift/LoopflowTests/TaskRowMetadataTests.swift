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
        let waiting = try session("waiting", availability: .available)
        let status = SessionProgramStatus(sessions: [waiting, reporting], unavailable: nil)
        #expect(status.label == state.rawValue.capitalized)
        #expect(status.help.contains("reporter"))
        #expect(status.help.contains("**literal**"))
        #expect(!status.help.contains("\u{202E}"))

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

    @Test("Live clients and unknown readings omit activity; shared Available remains visible")
    func inference() throws {
        let active = try session("active")
        let waiting = try session("waiting", availability: .available)
        #expect(SessionProgramStatus(sessions: [active], unavailable: nil).label == nil)
        #expect(SessionProgramStatus(sessions: [active, waiting], unavailable: nil).label == "Available · inferred")
        let unknown = try session("unknown", state: .unknown)
        let closed = try session("closed", state: .closed, availability: .available)
        #expect(SessionProgramStatus(sessions: [unknown, closed], unavailable: nil).label == nil)
        #expect(SessionProgramStatus(sessions: [active], unavailable: "Offline").label == nil)
    }

    @Test("Cleared reporter records do not revive inferred activity or Available")
    func clearDoesNotInfer() throws {
        let cleared = try session("cleared", availability: .available, status: .init(seen: true, records: []))
        #expect(SessionProgramStatus(sessions: [cleared], unavailable: nil).label == nil)
    }

    @Test("Saved readings cannot resurrect program reports or Available")
    func savedStatusIsUnknown() throws {
        let report = ProgramStatusReport(state: .blocked, id: nil, kind: .question, progress: nil,
                                         app: nil, title: nil, msg: "Proceed?")
        let record = try session("saved", availability: .available, status: .init(seen: true, records: [report]))
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
            let view = TaskRowMetadata(task: task, status: status)
            #expect(throws: (any Error).self) {
                try view.inspect().find(viewWithAccessibilityIdentifier: "work-program-status-\(task.id)")
            }
        }
        let unavailable = TaskRowMetadata(task: task, status: .unavailable("Offline"))
        let icon = try unavailable.inspect().find(viewWithAccessibilityIdentifier: "work-program-status-unavailable-\(task.id)")
        #expect(try icon.accessibilityLabel().string().contains("Offline"))
    }

    @Test("Selection preserves reported failure and shared Available semantics")
    func selectedStatus() throws {
        let task = try #require(roadmap().waves.first?.tasks.items[1])
        let error = ProgramStatusReport(state: .error, id: nil, kind: nil, progress: nil,
                                       app: nil, title: nil, msg: "Build failed")
        let reported = SessionProgramStatus(sessions: [try session("failed", status: .init(seen: true, records: [error]))], unavailable: nil)
        #expect(reported.tone == .failure)
        let selected = TaskRowMetadata(task: task, status: reported).workSelection(true)
        #expect(try selected.inspect().find(text: "Error").string() == "Error")
        let available = SessionProgramStatus(sessions: [try session("available", availability: .available)], unavailable: nil)
        #expect(available.tone == .available)
        #expect(available.label == "Available · inferred")
        #expect(ProgramStatusState.working.workTone == .running)
        #expect(ProgramStatusState.blocked.workTone == .warning)
        #expect(ProgramStatusState.done.workTone == .done)
    }

    @Test("Rows name the owner and Machine, or say the Task is unowned")
    func ownerLine() throws {
        let tasks = try roadmap().waves.flatMap(\.tasks.items)
        let owned = try #require(tasks.first { $0.runtime?.owner != nil })
        let view = TaskRowMetadata(task: owned, status: .unknown)
        #expect(try view.inspect().find(text: "Jack Heart on mini").string() == "Jack Heart on mini")
        let unowned = try #require(tasks.first { $0.runtime != nil && $0.runtime?.owner == nil })
        #expect(try TaskRowMetadata(task: unowned, status: .unknown).inspect().find(text: "Unowned").string() == "Unowned")
        let unknown = try JSONDecoder().decode(TaskOwner.self, from: Data("""
        {"person":null,"person_key":null,"machine":{"id":null,"label":null}}
        """.utf8))
        #expect(unknown.summary == "Someone · Machine unknown")
        let unnamed = try JSONDecoder().decode(TaskOwner.self, from: Data("""
        {"person":null,"person_key":"maya@example.com","machine":{"id":null,"label":"maya-mbp"}}
        """.utf8))
        #expect(unnamed.summary == "maya@example.com on maya-mbp")
    }

    @Test("Task identifiers remain visible without an external issue URL")
    func identifierWithoutLink() throws {
        let task = try #require(roadmap().waves.flatMap(\.tasks.items).first { $0.reference.issueUrl == nil })
        let view = TaskRowMetadata(task: task, status: .unknown)
        #expect(try view.inspect().find(text: task.task.identifier).string() == task.task.identifier)
        #expect(throws: (any Error).self) {
            try view.inspect().find(viewWithAccessibilityIdentifier: "work-task-link-\(task.id)")
        }
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
        let files = workspace.files(issue: task.task.identifier, cwd: record.cwd)
        let document = files.document("draft.md")
        document.editor.string = "unfinished"
        let layout = workspace.multiplexer.layout
        let selected = workspace.presentation.selectedSessionId
        let owner = WorkWorkspaceKey(repository: "repo", kind: .wave, id: snapshot.waves[0].wave.id)
        let member = fixtureViewContent(work: key, title: task.task.name)
        model.applySelectionPlans([
            fixtureViewPlan(owner: .init(repository: "repo", kind: .repository, id: "repo"), members: [member]),
            fixtureViewPlan(owner: owner, members: [member]),
        ], reason: "")
        model.select(owner.reference)
        #expect(model.sidebarTaskIDs == [task.id])
        var conversationOpened = false
        let view = WorkNavigator(model: model, onOpenSession: { _ in conversationOpened = true })

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
        #expect(model.selection == owner.reference)
        #expect(registry.selectedWork == owner)
        #expect(workspace.presentation.selectedSessionId == selected)
        #expect(workspace.multiplexer.layout == layout)
        #expect(files.document("draft.md") === document)
        #expect(document.editor.string == "unfinished")
        try inspected.find(ViewType.Button.self, where: {
            try $0.accessibilityIdentifier() == "work-task-\(task.id)"
        }).tap()
        #expect(!conversationOpened)
        #expect(model.isSelected(try #require(key.reference)))
        #expect(registry.selectedWork == key)
    }

    @Test("Headers name known providers and leave arbitrary shell titles as context")
    func programHeaders() {
        let codex = PaneProgramHeading(provider: "codex", title: "Review sidebar", directory: "/src/loopflow", fallback: "Session")
        #expect(codex.name == "Codex")
        #expect(codex.context == "Review sidebar · loopflow")
        #expect(codex.detail.contains("/src/loopflow"))
        #expect(PaneProgramHeading(provider: "claude", title: "Review", directory: nil, fallback: "Session").name == "Claude")
        let shell = PaneProgramHeading(provider: nil, title: "vim notes.md", directory: "/src/loopflow", fallback: "Shell")
        #expect(shell.name == "Shell")
        #expect(shell.context == "vim notes.md · loopflow")
    }

    private func roadmap() throws -> RoadmapSnapshot {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        return try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/roadmap_snapshot.json")))
    }

    private func session(_ id: String, state: SessionState = .active, availability: SessionAvailability? = nil,
                         status: ProgramStatusRecords? = nil) throws -> SessionRecord {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        var value = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/session.json"))) as? [String: Any])
        value["id"] = id
        value["title"] = id
        value["state"] = state.rawValue
        value["task_ids"] = ["ts_review00000000000000000000000000"]
        var workspace = try #require(value["workspace"] as? [String: Any])
        workspace["task_id"] = "ts_review00000000000000000000000000"
        value["workspace"] = workspace
        value["availability"] = availability?.rawValue ?? NSNull() as Any
        value["program_status"] = try status.map { try JSONSerialization.jsonObject(with: JSONEncoder().encode($0)) } ?? NSNull()
        return try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: value))
    }
}
#endif

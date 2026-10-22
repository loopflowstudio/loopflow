#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Console retained tools", .serialized)
@MainActor
struct ConsoleWorkspaceTests {
    @Test func togglesRetainShellTabsAndFileDrafts() throws {
        let workspace = fixtureWork(fixtureWorkspace("/task"))
        let store = workspace.multiplexer
        store.load(sessionId: "conversation", location: workspace.location)
        let conversation = store.focusedPane
        try workspace.newShell()
        let first = try #require(store.focusedPane.content.selectedShell)
        try workspace.newShell()
        let second = try #require(store.focusedPane.content.selectedShell)
        let shell = store.focusedPane
        #expect(first.id != second.id)
        #expect(store.layout.allPanes.count == 2)
        workspace.toggleFiles()
        let files = workspace.files(issue: "TASK", cwd: "/task")
        files.selection = "a.txt"
        let document = files.document("a.txt")
        document.editor.string = "unfinished"
        document.editor.setSelectedRange(NSRange(location: 2, length: 3))
        files.selection = "b.txt"
        let layout = store.layout
        for _ in 0..<2 {
            try workspace.toggleShell()
            workspace.toggleFiles()
        }
        #expect(store.layout == layout)
        #expect(store.collapsedPaneIds.isEmpty)
        #expect(store.layout.pane(for: conversation.id) == conversation)
        #expect(store.layout.pane(for: shell.id)?.content.selectedShell == second)
        store.selectShell(first.id)
        #expect(store.focusedPane.content.selectedShell == first)
        #expect(workspace.files(issue: "TASK", cwd: "/task") === files)
        #expect(files.tabs.map(\.path) == ["a.txt", "b.txt"])
        #expect(document.editor.string == "unfinished")
        #expect(document.editor.selectedRange() == NSRange(location: 2, length: 3))
    }

    @Test(arguments: [WorkWorkspaceKey.Kind.repository, .wave])
    func scopeNavigationCannotAllocateTools(kind: WorkWorkspaceKey.Kind) throws {
        let workspace = SessionsWorkspace(key: .init(repository: "repo", kind: kind, id: "scope"),
            title: "Scope", location: fixtureWorkspace("/repo"), localMachineId: fixtureMachineId)
        let before = workspace.multiplexer.layout
        workspace.toggleFiles()
        try workspace.toggleShell()
        #expect(throws: RegistryQueryError.self) { try workspace.newShell() }
        #expect(workspace.multiplexer.layout == before)
    }

    @Test func previewsExpireAndNeverCombineOtherSessionStatus() throws {
        let now = Date(timeIntervalSince1970: 1000)
        let working = try record(age: 10)
        #expect(SessionProgramStatus.unknown.preview(now: now, fallback: "brief", sessions: [working]).text == "Current output")
        let stale = try record(age: 31)
        #expect(SessionProgramStatus.unknown.preview(now: now, fallback: "brief", sessions: [stale]).text == "Summary · brief")
        let available = try record(age: 300, available: true)
        #expect(SessionProgramStatus.available(available).preview(now: now, fallback: "brief").text == "Current output")
        let missing = try record(age: nil, available: true)
        #expect(SessionProgramStatus.available(missing).preview(now: now, fallback: "brief", sessions: [working]).text == "Summary · brief")
        #expect(SessionProgramStatus.unavailable("Offline").preview(now: now, fallback: "brief", sessions: [working]).text == "Summary · brief")
        let replaced = try record(age: 10, process: nil)
        #expect(SessionProgramStatus.unknown.preview(now: now, fallback: "brief", sessions: [replaced]).text == "Summary · brief")
    }

    @Test(arguments: [ProgramStatusState.idle, .done])
    func settledReportsUseSummary(state: ProgramStatusState) throws {
        let session = try record(age: 1)
        let report = ProgramStatusReport(state: state, id: nil, kind: nil, progress: nil,
            app: nil, title: nil, msg: nil)
        let preview = SessionProgramStatus.reported(session, report).preview(
            now: Date(timeIntervalSince1970: 1000), fallback: "brief")
        #expect(preview.text == "Summary · brief")
    }

    @Test func summaryUsesFirstLineOfBrief() {
        let preview = SessionProgramStatus.unknown.preview(now: Date(),
            fallback: "Keep unfinished input through Task switches.\n\nImplementation instructions follow.")
        #expect(preview.text == "Summary · Keep unfinished input through Task switches.")
    }

    private func record(age: Int?, available: Bool = false, process: String? = "agent") throws -> SessionRecord {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let data = try Data(contentsOf: root.appendingPathComponent("tests/fixtures/dto/sessions.json"))
        var value = try #require((JSONSerialization.jsonObject(with: data) as? [[String: Any]])?.first)
        value["state"] = "active"
        value["program_status"] = NSNull()
        value["availability"] = available ? "available" : NSNull()
        value["agent_process_id"] = process ?? NSNull() as Any
        value["preview"] = age.map { ["provider_thread": "thread", "provider_turn": "turn", "source": "codex",
            "observed_at": 1000 - $0, "text": "Current output"] as [String: Any] } ?? NSNull() as Any
        return try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: value))
    }
}
#endif

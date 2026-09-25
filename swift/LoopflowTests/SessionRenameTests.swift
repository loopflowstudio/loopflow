import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

/// Named Session drill-down: shared rename readback, retained rejections,
/// and no late publication into another Session or an older read.
@MainActor
@Suite struct SessionRenameTests {
    @Test("A held rename publishes Rust's name; an older poll cannot restore the old one")
    func heldRenameSurvivesNavigationAndPolling() async throws {
        let source = RenameSource(records: [
            try record("first", title: "review-design"),
            try record("second", title: "lyric-cadenza"),
        ])
        let model = PodiumModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        await model.refresh()
        await model.refreshSessions()
        model.navigation.selectedSessionId = "first"
        model.beginSessionRename(try #require(model.sessions.value?.first { $0.id == "first" }))
        model.navigation.renaming?.text = "Launch design"

        await source.holdNext("rename")
        let rename = Task { await model.commitSessionRename() }
        await source.waitUntilHeld("rename")
        #expect(model.navigation.renaming?.submitting == true)

        // The human moves on while the rename is in flight, and a poll that
        // started before the write is also held.
        model.navigation.selectedSessionId = "second"
        #expect(model.navigation.renaming?.sessionId == "first")
        await source.holdNext("list")
        let poll = Task { await model.refreshSessions() }
        await source.waitUntilHeld("list")

        await source.release("rename")
        await rename.value
        #expect(title(model, "first") == "Launch design")
        #expect(model.sessions.value?.first { $0.id == "first" }?.titleSource == .human)
        #expect(model.navigation.renaming == nil)
        #expect(model.navigation.selectedSessionId == "second")

        await source.release("list")
        await poll.value
        #expect(title(model, "first") == "Launch design")
        #expect(title(model, "second") == "lyric-cadenza")
        #expect(await source.renames == [["session", "rename", "--json", "--", "first", "Launch design"]])
    }

    @Test("A rejected rename keeps the typed name and error on that Session only")
    func rejectionRetainsDraft() async throws {
        let source = RenameSource(records: [
            try record("first", title: "review-design"),
            try record("second", title: "lyric-cadenza"),
        ])
        await source.reject("Session name must be at most 80 characters")
        let model = PodiumModel(query: RegistryQuery { args, _ in try await source.read(args) }, repoPath: "/src/loopflow")
        await model.refreshSessions()
        model.navigation.selectedSessionId = "first"
        model.beginSessionRename(try #require(model.sessions.value?.first { $0.id == "first" }))
        model.navigation.renaming?.text = "An overlong name"
        await model.commitSessionRename()

        let draft = try #require(model.navigation.renaming)
        #expect(draft.sessionId == "first")
        #expect(draft.text == "An overlong name")
        #expect(draft.submitting == false)
        #expect(draft.error?.contains("at most 80") == true)
        #expect(title(model, "first") == "review-design")

        // Leaving the Session abandons the unsubmitted edit rather than
        // carrying it onto the next Session.
        model.navigation.selectedSessionId = "second"
        #expect(model.navigation.renaming == nil)
    }

    @Test("The breadcrumb drills Wave → Task → exact Session, with siblings as the final choice")
    func breadcrumbNamesTheExactSession() throws {
        let roadmap = try JSONDecoder().decode(RoadmapSnapshot.self, from: Data(roadmapJSON().utf8))
        let task = WorkReference.task(id: "ts_review00000000000000000000000000")
        let one = WorkspaceProjection(roadmaps: roadmap.waves, sessions: [try record("first", title: "review-design", work: task)])
        let single = try #require(one.breadcrumb(selection: nil, sessionId: "first"))
        #expect(single.wave?.roadmap.wave.name == "product")
        #expect(single.task?.task.task.identifier == "W2-131")
        #expect(single.session?.id == "first")
        #expect(single.siblings.map(\.id) == ["first"])

        let many = WorkspaceProjection(roadmaps: roadmap.waves, sessions: [
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

    private func title(_ model: PodiumModel, _ id: String) -> String? {
        model.sessions.value?.first { $0.id == id }?.title
    }
}

func renameFixtureRecord(
    _ id: String, title: String, source: String = "generated", work: WorkReference? = nil
) throws -> SessionRecord {
    let workJSON = String(decoding: try JSONEncoder().encode(work), as: UTF8.self)
    return try JSONDecoder().decode(SessionRecord.self, from: Data("""
    {"id":"\(id)","run_id":"run_\(id)","kind":"interactive","work":\(workJSON),"title":"\(title)",
     "detail":"codex","cwd":"/src/loopflow","state":"closed","wave_id":null,"work_path":null,
     "actions":\(sessionActionFixtureJSON(kind: "interactive", state: "closed")),
     "ready_summary":null,"title_source":"\(source)","flow_membership":{"kind":"independent"},
     "terminal_ids":[],"open_argv":["lf","session","open","\(id)"]}
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

/// A shared-CLI stand-in: `session list` returns the records as they were
/// when the read began; `session rename` returns the authoritative record.
private actor RenameSource {
    private var records: [SessionRecord]
    private var rejection: String?
    private var holds: Set<String> = []
    private var held: [String: CheckedContinuation<Void, Never>] = [:]
    private var waiting: [String: CheckedContinuation<Void, Never>] = [:]
    private(set) var renames: [[String]] = []

    init(records: [SessionRecord]) { self.records = records }

    func reject(_ message: String) { rejection = message }
    func holdNext(_ verb: String) { holds.insert(verb) }

    func waitUntilHeld(_ verb: String) async {
        if held[verb] != nil { return }
        await withCheckedContinuation { waiting[verb] = $0 }
    }

    func release(_ verb: String) {
        held.removeValue(forKey: verb)?.resume()
    }

    func read(_ args: [String]) async throws -> String {
        if args == ["roadmap", "--all", "--json"] {
            return try roadmapJSON()
        }
        guard args.count >= 2, args[0] == "session" else { return "[]" }
        let verb = args[1]
        let snapshot = records
        if holds.remove(verb) != nil {
            await withCheckedContinuation { continuation in
                held[verb] = continuation
                waiting.removeValue(forKey: verb)?.resume()
            }
        }
        switch verb {
        case "list":
            return String(decoding: try JSONEncoder().encode(snapshot), as: UTF8.self)
        case "rename":
            renames.append(args)
            if let rejection { throw RegistryQueryError(rejection) }
            let id = args[4], name = args[5]
            let renamed = try renameFixtureRecord(id, title: name, source: "human")
            records = records.map { $0.id == id ? renamed : $0 }
            return String(decoding: try JSONEncoder().encode(renamed), as: UTF8.self)
        default:
            return "[]"
        }
    }
}

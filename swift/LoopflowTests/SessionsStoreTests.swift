#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Sessions store")
@MainActor
struct SessionsStoreTests {
    @Test("Filtered refresh retains Sessions without creating local terminals")
    func filteredRefreshRetainsSessions() throws {
        let store = SessionsStore(repoPath: "/tmp/repo")
        store.reconcile(try records([
            session(id: "a", state: "waiting"),
            session(id: "b", state: "active"),
        ]))

        #expect(item(store, "a")?.state == .pending)
        #expect(item(store, "b")?.surface == nil)

        store.reconcile(try records([session(id: "a", state: "waiting")]))

        #expect(store.sessions.map(\.id) == ["a", "b"])
        #expect(item(store, "a")?.state == .pending)
    }

    @Test("selecting one waiting Task recovers only that terminal")
    func opensOnlyTheSelectedSession() async throws {
        let store = SessionsStore(
            repoPath: "/tmp/repo",
            query: RegistryQuery { args, _ in
                #expect(args.first == "session")
                #expect(args.dropFirst().first == "connect")
                return session(id: args[2], state: "active")
            }
        )
        store.reconcile(try records([
            session(id: "first", state: "waiting"),
            session(id: "second", state: "waiting"),
        ]))

        await store.select("second")

        #expect(item(store, "first")?.state == .pending)
        #expect(item(store, "second")?.surface?.openArgv.suffix(3) == ["session", "connect", "second"])
    }

    @Test("Selecting an active Session leaves its other terminal running until Move here")
    func interactiveSelectionRequiresExplicitMove() async throws {
        let store = SessionsStore(
            repoPath: "/tmp/repo",
            query: RegistryQuery { args, _ in
                #expect(args == ["session", "connect", "native", "--json", "--replace"])
                return session(id: "native", state: "closed")
            }
        )
        store.reconcile(try records([
            session(id: "native", state: "active"),
        ]))

        await store.select("native")
        #expect(item(store, "native")?.surface == nil)
        #expect(item(store, "native")?.state == .elsewhere)

        await store.moveHere("native")

        #expect(item(store, "native")?.surface != nil)
    }

    @Test("Explicit reopen revalidates a cached observation failure", arguments: ["available", "refused", "unavailable"])
    func reopenRevalidatesCachedFailure(result: String) async throws {
        let reason = "Session client observation unavailable"
        let record = session(id: "native", state: "closed", kind: "conversation")
        let blocked = record.replacingOccurrences(of: "\"unavailable_reason\":null", with: "\"unavailable_reason\":\"\(reason)\"")
        let store = SessionsStore(repoPath: "/tmp/repo", query: RegistryQuery { _, _ in
            if result == "refused" { throw RegistryQueryError(reason) }
            return result == "unavailable" ? blocked : record
        })
        store.reconcile(try records([blocked]))
        #expect(item(store, "native")?.record.action(.open)?.unavailableReason == reason)

        await store.select("native")

        if result == "available" {
            #expect(item(store, "native")?.state == .prepared)
            #expect(item(store, "native")?.surface?.openArgv == ["lf", "session", "connect", "native"])
        } else {
            #expect(item(store, "native")?.surface == nil)
            #expect(item(store, "native")?.error?.contains(reason) == true)
        }
    }

    @Test("Polling preserves the prepared interactive launch command", arguments: [false, true])
    func reconcilePreservesPreparedLaunch(replacing: Bool) async throws {
        let store = SessionsStore(
            repoPath: "/tmp/repo",
            query: RegistryQuery { _, _ in
                session(id: "native", state: "closed", replacing: replacing)
            }
        )
        store.reconcile(try records([
            session(id: "native", state: replacing ? "active" : "closed"),
        ]))

        if replacing {
            await store.moveHere("native")
        } else {
            await store.select("native")
        }
        let prepared = try #require(item(store, "native")?.surface)
        #expect(prepared.openArgv.contains("--replace") == replacing)
        store.reconcile(try records([
            session(id: "native", state: "active"),
        ]))

        #expect(item(store, "native")?.state == .prepared)
        #expect(item(store, "native")?.surface?.openArgv == prepared.openArgv)
        store.reconcile([])
        store.reconcile([])
        #expect(item(store, "native")?.state == .prepared)
        #expect(item(store, "native")?.surface?.openArgv == prepared.openArgv)
    }

    @Test("An externally killed terminal reclassifies as active elsewhere")
    func externalKillReclassifies() async throws {
        let store = SessionsStore(
            repoPath: "/tmp/repo",
            query: RegistryQuery { _, _ in
                session(id: "native", state: "active")
            }
        )
        store.reconcile(try records([
            session(id: "native", state: "active"),
        ]))
        await store.moveHere("native")
        store.recordPaneLive("native")
        #expect(item(store, "native")?.state == .live)

        store.noteSurfaceClosed(.shell("native"))
        #expect(item(store, "native")?.state == .live)

        store.noteSurfaceClosed(.session("native"))

        #expect(item(store, "native")?.state == .elsewhere)
        #expect(item(store, "native")?.surface == nil)
    }

    @Test("A failed open stays visible across polling until superseded")
    func failedOpenSurvivesPolling() async throws {
        let store = SessionsStore(
            repoPath: "/tmp/repo",
            query: RegistryQuery { args, _ in
                #expect(args.contains("connect"))
                return "not json"
            }
        )
        store.reconcile(try records([
            session(id: "native", state: "closed"),
        ]))

        await store.select("native")
        #expect(item(store, "native")?.error != nil)

        store.reconcile(try records([
            session(id: "native", state: "closed"),
        ]))
        #expect(item(store, "native")?.error != nil)

        store.reconcile(try records([
            session(id: "native", state: "active"),
        ]))
        #expect(item(store, "native")?.state == .elsewhere)
    }


    @Test("Retained review feedback stays visible")
    func retainedFeedbackDoesNotDisappear() throws {
        let store = SessionsStore(repoPath: "/tmp/repo")
        store.reconcile(try records([session(id: "review", state: "waiting")]))

        #expect(store.sessions.map(\.id) == ["review"])
        #expect(store.sessions.first?.record.attention == .waiting)
        #expect(store.sessions.first?.record.readySummary == "Ready for review")
    }


    @Test("Session actions use the main repository for a linked worktree")
    func sessionStoreUsesMainRepository() throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("session-scope-\(UUID().uuidString)", isDirectory: true)
        let main = root.appendingPathComponent("loopflow", isDirectory: true)
        let worktree = root.appendingPathComponent("loopflow.feature", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try FileManager.default.createDirectory(at: main, withIntermediateDirectories: true)
        try runGit(["init", "-q"], at: main)
        try runGit(
            [
                "-c", "user.email=t@t", "-c", "user.name=t",
                "commit", "-q", "--allow-empty", "-m", "init",
            ],
            at: main
        )
        try runGit(["worktree", "add", "-q", worktree.path], at: main)

        let store = SessionsStore(repoPath: worktree.path)

        #expect(
            URL(fileURLWithPath: store.repoPath).resolvingSymlinksInPath().path
                == main.resolvingSymlinksInPath().path
        )
    }
}

@MainActor
private func item(_ store: SessionsStore, _ id: String) -> SessionItem? {
    store.sessions.first { $0.id == id }
}

private func records(_ entries: [String]) throws -> [SessionRecord] {
    try JSONDecoder().decode(
        [SessionRecord].self,
        from: Data("[\(entries.joined(separator: ","))]".utf8)
    )
}

private func session(id: String, state: String, replacing: Bool = false) -> String {
    // A conversation waiting on a person has no client; `waiting` is its attention.
    let wire = state == "waiting" ? "unknown" : state
    return """
    {
      "id": "\(id)", "run_id": "\(id)", "interactive": true,
      "work": { "kind": "task", "id": "task-\(id)" },
      "title": "Design the control surface",
      "detail": "review-design",
      "cwd": "/tmp/repo.\(id)",
      "state": "\(wire)", "attention": \(state == "waiting" ? "\"waiting\"" : "null"),
      "ready_summary": \(state == "waiting" ? "\"Ready for review\"" : "null"),
      "work_path": "product / Desktop / LOO-291",
      "actions": \(sessionActionFixtureJSON(state: wire)),
      "title_source": "generated", "task_primary": false, "flow_membership": {"kind": "independent"}, "task_ids": ["task-\(id)"], "terminal_ids": [],
      "open_argv": ["lf", "session", "connect", "\(id)"\(replacing ? ", \"--replace\"" : "")]
    }
    """
}

private func runGit(_ args: [String], at directory: URL) throws {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
    process.arguments = ["git", "-C", directory.path] + args
    process.standardOutput = Pipe()
    process.standardError = Pipe()
    try process.run()
    process.waitUntilExit()
    try #require(process.terminationStatus == 0, "git \(args.joined(separator: " "))")
}

private actor SessionCalls {
    private(set) var values: [[String]] = []

    func append(_ value: [String]) {
        values.append(value)
    }
}
#endif

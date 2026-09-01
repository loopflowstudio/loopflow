#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Sessions store")
@MainActor
struct SessionsStoreTests {
    @Test("refresh reads Task flow sessions in the opened repository")
    func refreshUsesRepoScope() async {
        let store = SessionsStore(
            scope: .repo("/tmp/scoped-repo"),
            query: RegistryQuery { args, cwd in
                #expect(args == ["session", "list", "--json"])
                #expect(cwd == "/tmp/scoped-repo")
                return "[\(session(id: "review", state: "active"))]"
            }
        )

        await store.refresh()

        #expect(store.hasLoaded)
        #expect(store.sessions.map(\.id) == ["review"])
        #expect(store.sessions.first?.label == "Design the control surface")
        #expect(store.sessions.first?.step == "review-design")
    }

    @Test("reconcile adopts liveness and removes Tasks that advanced")
    func reconcileTracksTheTaskPlayhead() throws {
        let store = SessionsStore(scope: .repo("/tmp/repo"))
        store.reconcile(try records([
            session(id: "a", state: "waiting"),
            session(id: "b", state: "active"),
        ]))

        #expect(item(store, "a")?.state == .pending)
        #expect(item(store, "b")?.surface != nil)

        store.reconcile(try records([session(id: "a", state: "active")]))

        #expect(store.sessions.map(\.id) == ["a"])
        #expect(item(store, "a")?.surface?.id == "a")
    }

    @Test("selecting one waiting Task recovers only that terminal")
    func opensOnlyTheSelectedSession() async throws {
        let store = SessionsStore(
            scope: .repo("/tmp/repo"),
            query: RegistryQuery { args, _ in
                #expect(args.first == "session")
                #expect(args.dropFirst().first == "open")
                return session(id: args[2], state: "active")
            }
        )
        store.reconcile(try records([
            session(id: "first", state: "waiting"),
            session(id: "second", state: "waiting"),
        ]))

        let opened = await store.select("second")

        #expect(opened?.id == "second")
        #expect(item(store, "first")?.state == .pending)
        #expect(item(store, "second")?.surface?.openArgv.suffix(3) == ["session", "open", "second"])
    }

    @Test("Selecting an interactive Session replaces its active provider client")
    func interactiveSelectionUsesReplace() async throws {
        let store = SessionsStore(
            scope: .repo("/tmp/repo"),
            query: RegistryQuery { args, _ in
                #expect(args == ["session", "open", "native", "--json", "--replace"])
                return session(id: "native", state: "closed", kind: "interactive")
            }
        )
        store.reconcile(try records([
            session(id: "native", state: "active", kind: "interactive"),
        ]))

        let opened = await store.select("native")

        #expect(opened?.id == "native")
        #expect(item(store, "native")?.surface != nil)
    }

    @Test("Polling preserves an open interactive Session terminal")
    func reconcilePreservesInteractiveSurface() async throws {
        let store = SessionsStore(
            scope: .repo("/tmp/repo"),
            query: RegistryQuery { args, _ in
                #expect(args == ["session", "open", "native", "--json", "--replace"])
                return session(id: "native", state: "closed", kind: "interactive")
            }
        )
        store.reconcile(try records([
            session(id: "native", state: "closed", kind: "interactive"),
        ]))

        _ = await store.select("native")
        store.reconcile(try records([
            session(id: "native", state: "active", kind: "interactive"),
        ]))

        #expect(item(store, "native")?.surface?.state == .active)
    }

    @Test("Completing an interactive Session removes it from Sessions")
    func completionRemovesInteractiveSession() async throws {
        let calls = SessionCalls()
        let store = SessionsStore(
            scope: .repo("/tmp/repo"),
            query: RegistryQuery { args, cwd in
                await calls.append(args)
                #expect(cwd == "/tmp/repo")
                if args == ["session", "complete", "native"] {
                    return "Session native completed"
                }
                #expect(args == ["session", "list", "--json"])
                return "[]"
            }
        )
        store.reconcile(try records([
            session(id: "native", state: "active", kind: "interactive"),
        ]))

        let completed = await store.complete("native")

        #expect(completed)
        #expect(store.sessions.isEmpty)
        #expect(await calls.values == [
            ["session", "complete", "native"],
            ["session", "list", "--json"],
        ])
    }

    @Test("Completing a ready Ask resumes its caller")
    func completionRemovesAskSession() async throws {
        let calls = SessionCalls()
        let store = SessionsStore(
            scope: .repo("/tmp/repo"),
            query: RegistryQuery { args, cwd in
                await calls.append(args)
                #expect(cwd == "/tmp/repo")
                if args == ["session", "complete", "ask"] {
                    return "Ask session completed: Ready for review"
                }
                #expect(args == ["session", "list", "--json"])
                return "[]"
            }
        )
        store.reconcile(try records([
            session(id: "ask", state: "ready", kind: "ask"),
        ]))

        let completed = await store.complete("ask")

        #expect(completed)
        #expect(store.sessions.isEmpty)
        #expect(await calls.values == [
            ["session", "complete", "ask"],
            ["session", "list", "--json"],
        ])
    }

    @Test("A ready FlowStep stays visible until its decision")
    func readyDoesNotDisappear() throws {
        let store = SessionsStore(scope: .repo("/tmp/repo"))
        store.reconcile(try records([session(id: "review", state: "ready")]))

        #expect(store.sessions.map(\.id) == ["review"])
        #expect(store.sessions.first?.statusLabel == "READY")
        #expect(store.sessions.first?.record.readySummary == "Ready for review")
    }

    @Test("Only a FlowStep decision removes a ready FlowStep")
    func resolutionNamesTheSession() async throws {
        let calls = SessionCalls()
        let store = SessionsStore(
            scope: .repo("/tmp/repo"),
            query: RegistryQuery { args, cwd in
                await calls.append(args)
                #expect(cwd == "/tmp/repo")
                if args == ["session", "approve", "review", "Ready for review"] {
                    return "Task FlowStep approved"
                }
                #expect(args == ["session", "list", "--json"])
                return "[]"
            }
        )
        store.reconcile(try records([session(id: "review", state: "ready")]))

        let decided = await store.decideFlow(
            "review",
            approving: true,
            text: "Ready for review"
        )

        #expect(decided)
        #expect(store.sessions.isEmpty)
        #expect(await calls.values == [
            ["session", "approve", "review", "Ready for review"],
            ["session", "list", "--json"],
        ])
    }

    @Test("Session scope collapses a linked worktree to its main repository")
    func sessionScopeUsesMainRepository() throws {
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

        let scope = SessionScope.repo(worktree.path).resolvingRepository()

        #expect(
            URL(fileURLWithPath: scope.repoPath).resolvingSymlinksInPath().path
                == main.resolvingSymlinksInPath().path
        )
        #expect(scope.label == "loopflow")
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

private func session(id: String, state: String, kind: String = "flow") -> String {
    """
    {
      "id": "\(id)",
      "kind": "\(kind)",
      "work": { "kind": "task", "id": "task-\(id)" },
      "title": "Design the control surface",
      "detail": "review-design",
      "cwd": "/tmp/repo.\(id)",
      "state": "\(state)",
      "ready_summary": \(state == "ready" ? "\"Ready for review\"" : "null"),
      "open_argv": ["lf", "session", "open", "\(id)"]
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

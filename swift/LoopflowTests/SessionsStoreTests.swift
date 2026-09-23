#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Sessions store")
@MainActor
struct SessionsStoreTests {
    @Test("reconcile adopts liveness and removes Tasks that advanced")
    func reconcileTracksTheTaskPlayhead() throws {
        let store = SessionsStore(repoPath: "/tmp/repo")
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
            repoPath: "/tmp/repo",
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

    @Test("Selecting an active Session leaves its other terminal running until Move here")
    func interactiveSelectionRequiresExplicitMove() async throws {
        let store = SessionsStore(
            repoPath: "/tmp/repo",
            query: RegistryQuery { args, _ in
                #expect(args == ["session", "open", "native", "--json", "--replace"])
                return session(id: "native", state: "closed", kind: "interactive")
            }
        )
        store.reconcile(try records([
            session(id: "native", state: "active", kind: "interactive"),
        ]))

        let opened = await store.select("native")
        #expect(opened == nil)
        #expect(item(store, "native")?.state == .elsewhere)

        let moved = await store.moveHere("native")

        #expect(moved?.id == "native")
        #expect(item(store, "native")?.surface != nil)
    }

    @Test("Polling preserves the prepared interactive launch command", arguments: [false, true])
    func reconcilePreservesPreparedLaunch(replacing: Bool) async throws {
        let store = SessionsStore(
            repoPath: "/tmp/repo",
            query: RegistryQuery { _, _ in
                session(id: "native", state: "closed", kind: "interactive", replacing: replacing)
            }
        )
        store.reconcile(try records([
            session(id: "native", state: replacing ? "active" : "closed", kind: "interactive"),
        ]))

        let prepared = if replacing {
            await store.moveHere("native")
        } else {
            await store.select("native")
        }
        #expect(prepared?.openArgv.contains("--replace") == replacing)
        store.reconcile(try records([
            session(id: "native", state: "active", kind: "interactive"),
        ]))

        #expect(item(store, "native")?.state == .prepared)
        #expect(item(store, "native")?.surface?.openArgv == prepared?.openArgv)
    }

    @Test("An externally killed terminal reclassifies as active elsewhere")
    func externalKillReclassifies() async throws {
        let store = SessionsStore(
            repoPath: "/tmp/repo",
            query: RegistryQuery { _, _ in
                session(id: "native", state: "active", kind: "interactive")
            }
        )
        store.reconcile(try records([
            session(id: "native", state: "active", kind: "interactive"),
        ]))
        _ = await store.moveHere("native")
        store.recordPaneLive("native")
        #expect(item(store, "native")?.state == .live)

        store.noteSurfaceClosed(.shell("native"))
        store.noteSurfaceClosed(.taskTerminal("native"))
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
                #expect(args.contains("open"))
                return "not json"
            }
        )
        store.reconcile(try records([
            session(id: "native", state: "closed", kind: "interactive"),
        ]))

        _ = await store.select("native")
        #expect(item(store, "native")?.error != nil)

        store.reconcile(try records([
            session(id: "native", state: "closed", kind: "interactive"),
        ]))
        #expect(item(store, "native")?.error != nil)

        store.reconcile(try records([
            session(id: "native", state: "active", kind: "interactive"),
        ]))
        #expect(item(store, "native")?.state == .elsewhere)
    }

    @Test("Completing an interactive Session removes it from Sessions")
    func completionRemovesInteractiveSession() async throws {
        let calls = SessionCalls()
        let store = SessionsStore(
            repoPath: "/tmp/repo",
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
        ])
    }

    @Test("Completing a ready Ask resumes its caller")
    func completionRemovesAskSession() async throws {
        let calls = SessionCalls()
        let store = SessionsStore(
            repoPath: "/tmp/repo",
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
        ])
    }

    @Test("A ready review stays visible until completion")
    func readyDoesNotDisappear() throws {
        let store = SessionsStore(repoPath: "/tmp/repo")
        store.reconcile(try records([session(id: "review", state: "ready")]))

        #expect(store.sessions.map(\.id) == ["review"])
        #expect(store.sessions.first?.statusLabel == "READY")
        #expect(store.sessions.first?.record.readySummary == "Ready for review")
    }

    @Test("Completing a review removes its Session")
    func resolutionNamesTheSession() async throws {
        let calls = SessionCalls()
        let store = SessionsStore(
            repoPath: "/tmp/repo",
            query: RegistryQuery { args, cwd in
                await calls.append(args)
                #expect(cwd == "/tmp/repo")
                if args == ["session", "complete", "review"] {
                    return "Review feedback returned"
                }
                #expect(args == ["session", "list", "--json"])
                return "[]"
            }
        )
        store.reconcile(try records([session(id: "review", state: "ready")]))

        let decided = await store.complete("review")

        #expect(decided)
        #expect(store.sessions.isEmpty)
        #expect(await calls.values == [
            ["session", "complete", "review"],
        ])
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

private func session(id: String, state: String, kind: String = "flow", replacing: Bool = false) -> String {
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
      "open_argv": ["lf", "session", "open", "\(id)"\(replacing ? ", \"--replace\"" : "")]
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

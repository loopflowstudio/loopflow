#if os(macOS)
import AppKit
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

/// Warm Task conversations run before anyone looks at them: a prepared Session's
/// terminal starts from the store, and whatever pane later shows it adopts the
/// running view. Load prefetch is what starts the provider.
@Suite("Warm conversations start unmounted")
@MainActor
struct WarmPaneMountTests {
    #if canImport(GhosttyKit)
    @Test("A prepared Session starts its terminal before any pane shows it")
    func preparedSessionStartsUnmounted() async throws {
        _ = NSApplication.shared
        let pool = GhosttySurfacePool(runtime: .shared)
        let store = SessionsStore(repoPath: NSTemporaryDirectory(), query: RegistryQuery { _, _ in
            throw RegistryQueryError("Nothing is read here")
        }, surfaces: pool)
        let record = try warmSession(id: "warm-unmounted", argv: ["/bin/cat"])
        let terminal = TerminalIdentity.session(record.id, machineId: fixtureMachineId)
        defer { pool.release(terminal) }
        store.reconcile([record])
        store.adoptPreparation(record)
        #expect(store.sessions.first?.state == .prepared)
        let view = pool.view(for: terminal)
        try #require(view.surface != nil)
        #expect(view.window == nil)
        #expect(view.frame.width > 0 && view.frame.height > 0)
        #expect(store.localTerminal(for: record) == terminal)
    }
    @Test("Refresh retires a paused viewer before preparing its replacement, including overlapping requests")
    func refreshWaitsForPausedViewerRetirement() async throws {
        _ = NSApplication.shared
        let pool = GhosttySurfacePool(runtime: .shared)
        let record = try warmSession(id: "refresh-unmounted", argv: ["/bin/cat"])
        let terminal = TerminalIdentity.session(record.id, machineId: fixtureMachineId)
        let old = pool.view(for: terminal)
        let retired: @MainActor @Sendable () -> Bool = { old.childExited }
        let store = SessionsStore(repoPath: NSTemporaryDirectory(), query: RegistryQuery { args, _ in
            guard args == ["session", "resume", record.id, "--prepare", "--json"] else { throw RegistryQueryError("unexpected argv: \(args)") }
            let oldRetired = await retired()
            #expect(oldRetired)
            return String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        }, surfaces: pool)
        defer {
            pool.view(for: terminal).destroySurface()
            pool.release(terminal)
            old.destroySurface()
        }
        store.reconcile([record])
        store.adoptPreparation(record)
        try #require(old.surface != nil)
        #expect(!old.connectionAcceptsInput)
        let overlapping = Task { await store.refresh(record.id) }
        await store.refresh(record.id)
        await overlapping.value
        #expect(old.childExited)
        #expect(old.surface == nil)
        #expect(pool.view(for: terminal) !== old)
        #expect(store.sessions.first?.state == .prepared)
        #expect(store.attachment(record.id) == .opening)
    }

    @Test("Opening again replaces an ended viewer")
    func openingReplacesEndedViewer() async throws {
        _ = NSApplication.shared
        let pool = GhosttySurfacePool(runtime: .shared)
        let reading = #"{"generation":1,"state":"ended","native_ready":false,"message":null}"#
        let expected = DesktopOpening.Status.failed
        let record = try warmSession(id: "displaced", argv: ["/bin/sh", "-c",
            "(printf '%s\\n' '\(reading)'; sleep 30) | nc -U \"$LF_SERVER_STATUS\" & exec cat"])
        let terminal = TerminalIdentity.session(record.id, machineId: fixtureMachineId)
        let store = SessionsStore(repoPath: NSTemporaryDirectory(), query: RegistryQuery { args, _ in
            return String(decoding: try JSONEncoder().encode(record), as: UTF8.self)
        }, surfaces: pool)
        store.reconcile([record])
        store.adoptPreparation(record)
        let old = pool.view(for: terminal)
        defer {
            pool.view(for: terminal).destroySurface()
            pool.release(terminal)
            old.destroySurface()
        }
        store.recordPaneLive(record.id)
        let deadline = ContinuousClock.now + .seconds(10)
        while store.attachment(record.id) != expected, ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        try #require(store.attachment(record.id) == expected)
        #expect(store.sessions.first?.state == .live)
        await store.select(record.id)
        #expect(old.surface == nil)
        #expect(pool.view(for: terminal) !== old)
        #expect(store.sessions.first?.state == .prepared)
    }
    #endif

    @Test("A store without a runtime prepares and shows but launches nothing")
    func runtimelessStoreOnlyPrepares() throws {
        let pool = GhosttySurfacePool()
        let store = SessionsStore(repoPath: NSTemporaryDirectory(), query: RegistryQuery { _, _ in
            throw RegistryQueryError("Nothing is read here")
        }, surfaces: pool)
        let record = try warmSession(id: "warm-headless", argv: ["/bin/cat"])
        store.reconcile([record])
        store.adoptPreparation(record)
        #expect(store.sessions.first?.state == .prepared)
        #expect(!pool.hasSurface(TerminalIdentity.session(record.id, machineId: fixtureMachineId)))
    }
}

/// A Task primary with a checkout on this Machine, prepared to start `argv`.
private func warmSession(id: String, argv: [String]) throws -> SessionRecord {
    let wire: [String: Any] = [
        "id": id, "run_id": id, "interactive": true,
        "work": ["kind": "task", "id": "task-\(id)"],
        "title": "Warm conversation", "detail": "warm",
        "cwd": NSTemporaryDirectory(),
        "workspace": ["machine_id": fixtureMachineId, "worktree": NSTemporaryDirectory(), "task_id": "task-\(id)", "unavailable": NSNull()],
        "state": "closed", "availability": NSNull(), "work_path": "warm",
        "action": ["label": "Open here", "help": "Open this Session in a terminal"],
        "blocked": false, "title_source": "generated", "ended": NSNull(), "start": NSNull(), "outlook": NSNull(), "status_line": NSNull(), "primary_scope": "task", "graph_membership": ["kind": "independent"],
        "task_ids": ["task-\(id)"], "agent_process_id": NSNull(), "turn": NSNull(), "raw": false, "terminal_ids": [String](),
        "open_argv": argv,
    ]
    return try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: wire))
}
#endif

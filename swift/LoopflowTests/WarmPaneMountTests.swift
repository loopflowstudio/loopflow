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
        store.adoptPreparation(record, intent: .start)
        #expect(store.sessions.first?.state == .prepared)
        let view = pool.view(for: terminal)
        try #require(view.surface != nil)
        #expect(view.window == nil)
        #expect(view.frame.width > 0 && view.frame.height > 0)
        #expect(store.localTerminal(for: record) == terminal)
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
        store.adoptPreparation(record, intent: .start)
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
        "state": "closed", "availability": NSNull(), "ready_summary": NSNull(), "work_path": "warm",
        "actions": [["kind": "open", "label": "Start conversation", "help": "Start the conversation", "unavailable_reason": NSNull()]],
        "blocked": false, "title_source": "generated", "task_primary": true, "graph_membership": ["kind": "independent"],
        "task_ids": ["task-\(id)"], "agent_process_id": NSNull(), "terminal_ids": [String](),
        "open_argv": argv,
    ]
    return try JSONDecoder().decode(SessionRecord.self, from: JSONSerialization.data(withJSONObject: wire))
}
#endif

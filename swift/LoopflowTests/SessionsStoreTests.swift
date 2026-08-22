#if os(macOS)
import Foundation
import Testing
@testable import Loopflow
@testable import LoopflowMac

// Reconcile Sessions against polls, open queued Asks serially, and
// survive a failed open. Behavior only — the injected RegistryQuery runner
// stands in for `lf`, so nothing spawns.
@Suite("Sessions store")
@MainActor
struct SessionsStoreTests {
    @Test("refresh runs the Ask query in the opened repository")
    func refreshUsesRepoScope() async throws {
        let store = SessionsStore(
            scope: .repo("/tmp/scoped-repo"),
            query: RegistryQuery { args, cwd in
                #expect(args == ["ask", "list", "--user", "--json"])
                #expect(cwd == "/tmp/scoped-repo")
                return "[\(entry(id: "scoped", attention: "claimed", surface: true))]"
            }
        )

        await store.refresh()

        #expect(store.hasLoaded)
        #expect(store.sessions.map(\.id) == ["scoped"])
    }

    @Test("reconcile adds, updates, and drops settled sessions")
    func reconcileAddsUpdatesRemoves() async throws {
        // Runner only ever needs to answer "ask open" if the opener races; a
        // surface is fine. The assertions below read synchronous post-reconcile
        // state, before the opener Task runs.
        let store = SessionsStore(
            scope: .repo("/tmp/repo"),
            query: RegistryQuery { _, _ in surfaceJSON(invId: "inv-x") }
        )

        store.reconcile(try records([
            entry(id: "a", attention: "queued", surface: false),
            entry(id: "b", attention: "claimed", surface: true, invId: "inv-b"),
        ]))
        #expect(store.sessions.count == 2)
        #expect(item(store, "a")?.state == .pending)  // no surface → awaits opener
        #expect(item(store, "b")?.surface != nil)      // surface present → attaches now

        // 'a' gains a surface on the next poll; 'b' settled and left the queue.
        store.reconcile(try records([
            entry(id: "a", attention: "claimed", surface: true, invId: "inv-a"),
        ]))
        #expect(store.sessions.map(\.id) == ["a"])
        #expect(item(store, "a")?.surface != nil)      // adopted the surface
    }

    @Test("queued Asks open one at a time, open-first, never twice")
    func opensSeriallyOpenFirst() async throws {
        let probe = OpenProbe()
        let store = SessionsStore(
            scope: .repo("/tmp/repo"),
            query: RegistryQuery { args, _ in
                guard args.count >= 3, args[1] == "open" else { return "[]" }
                let id = args[2]
                await probe.begin(id)
                try? await Task.sleep(nanoseconds: 15_000_000)
                await probe.end()
                return surfaceJSON(invId: "inv-\(id)")
            }
        )

        store.reconcile(try records([
            entry(id: "q1", attention: "queued", surface: false),
            entry(id: "q2", attention: "queued", surface: false),
            entry(id: "open1", attention: "claimed", surface: false),  // recover, no spawn
            entry(id: "q3", attention: "queued", surface: false),
        ]))

        await settle(store) { store.sessions.allSatisfy { $0.surface != nil } }

        #expect(await probe.maxConcurrent == 1)                    // strictly serial
        #expect(await probe.order.first == "open1")                // open-ones first
        #expect(await probe.counts.count == 4)                     // each opened…
        #expect(await probe.counts.values.allSatisfy { $0 == 1 })  // …exactly once
    }

    @Test("a failed open marks its session and the opener keeps going")
    func failureDoesNotStallBoard() async throws {
        let store = SessionsStore(
            scope: .repo("/tmp/repo"),
            query: RegistryQuery { args, _ in
                guard args.count >= 3, args[1] == "open" else { return "[]" }
                let id = args[2]
                try? await Task.sleep(nanoseconds: 5_000_000)
                if id == "bad" { throw StubError() }
                return surfaceJSON(invId: "inv-\(id)")
            }
        )

        store.reconcile(try records([
            entry(id: "bad", attention: "queued", surface: false),
            entry(id: "ok1", attention: "queued", surface: false),
            entry(id: "ok2", attention: "queued", surface: false),
        ]))

        await settle(store) {
            store.sessions.allSatisfy { $0.surface != nil || $0.error != nil }
        }

        #expect(item(store, "bad")?.error != nil)
        #expect(item(store, "ok1")?.surface != nil)
        #expect(item(store, "ok2")?.surface != nil)
    }
}

// MARK: - Helpers

private struct StubError: Error {}

private actor OpenProbe {
    private(set) var inFlight = 0
    private(set) var maxConcurrent = 0
    private(set) var order: [String] = []
    private(set) var counts: [String: Int] = [:]

    func begin(_ id: String) {
        inFlight += 1
        maxConcurrent = max(maxConcurrent, inFlight)
        order.append(id)
        counts[id, default: 0] += 1
    }

    func end() { inFlight -= 1 }
}

@MainActor
private func settle(_ store: SessionsStore, until: () -> Bool) async {
    for _ in 0..<400 {  // ~2s ceiling
        if until() { return }
        try? await Task.sleep(nanoseconds: 5_000_000)
    }
}

@MainActor
private func item(_ store: SessionsStore, _ id: String) -> SessionItem? {
    store.sessions.first { $0.id == id }
}

private func records(_ entries: [String]) throws -> [AskAttentionRecord] {
    let json = "[\(entries.joined(separator: ","))]"
    return try JSONDecoder().decode([AskAttentionRecord].self, from: Data(json.utf8))
}

private func entry(id: String, attention: String, surface: Bool, invId: String = "inv") -> String {
    let state = attention == "queued" ? "queued" : "claimed"
    let surfaceField = surface ? surfaceJSON(invId: invId) : "null"
    return """
    {
      "ask": {
        "id": "\(id)",
        "origin": { "work": { "kind": "task", "id": "task-\(id)" } },
        "target": { "kind": "user" },
        "request": { "kind": "intervention", "prompt": "Question \(id)" },
        "state": "\(state)",
        "active_invocation_id": null,
        "result": null,
        "terminal_author": null,
        "asked_at": "2026-07-19T22:00:01Z",
        "terminal_at": null
      },
      "surface": \(surfaceField),
      "attention": "\(attention)"
    }
    """
}

private func surfaceJSON(invId: String) -> String {
    """
    {
      "invocation": {
        "id": "\(invId)",
        "supervising_run_id": "run2",
        "answer_ask_id": null,
        "route": { "provider": "opaque", "model": null, "account_id": null },
        "surface": "terminal",
        "resume_token": null,
        "started_at": "2026-07-17T12:00:00Z",
        "ended_at": null
      },
      "run": {
        "id": "run2",
        "work": { "kind": "task", "id": "task5" },
        "epoch_id": "e",
        "home_id": "h",
        "runtime_generation": 8,
        "state": "active",
        "trigger": { "kind": "user" },
        "retry_of": null,
        "containment": { "kind": "tmux", "name": "lf-task" },
        "cwd": "/tmp",
        "created_at": "2026-07-17T11:59:59Z",
        "started_at": "2026-07-17T12:00:00Z",
        "ended_at": null
      },
      "work": { "kind": "task", "id": "task5" },
      "wave_id": "00000000-0000-4000-8000-000000000006",
      "home_route": "jack@local",
      "handback": null,
      "attach_argv": ["tmux", "attach-session", "-t", "lf-task"],
      "current": {
        "state": "live",
        "reason": "the owning Home verified the supervising Run",
        "liveness": {
          "state": "present",
          "observed_at": "2026-07-17T12:00:01Z",
          "fresh": true
        }
      }
    }
    """
}
#endif

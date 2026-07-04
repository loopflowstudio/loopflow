// Streaming contract for WaveChatConnection: the server re-sends an
// in-progress turn whole under the same id as it grows, then a terminal frame
// at finalization. Repeated same-id frames must update in place, flip status
// running → completed/failed/interrupted, and keep the thread order
// deterministic.

import Foundation
import Testing
@testable import LoopflowCore

@MainActor
@Suite("WaveChat streaming upsert")
struct WaveChatConnectionTests {
    private func connection() -> WaveChatConnection {
        WaveChatConnection(repoPath: "/tmp/nowhere", waveName: "ship")
    }

    /// A wire-shaped `turn` SSE payload. `text` is spliced raw, so escape
    /// inside it (e.g. `\\n`) as the server would.
    private func frame(id: String, role: String = "assistant", text: String, status: String, items: String = "[]") -> String {
        "{\"id\":\"\(id)\",\"role\":\"\(role)\",\"text\":\"\(text)\",\"status\":\"\(status)\",\"items\":\(items),\"created_at\":\"2026-07-04T00:00:00Z\"}"
    }

    @Test("repeated same-id frames grow the turn in place and finalize it")
    func sameIdFramesUpdateInPlace() {
        let conn = connection()
        conn.handle(event: "turn", data: frame(id: "turn-1", role: "user", text: "status?", status: "completed"))
        conn.handle(event: "turn", data: frame(id: "turn-2", text: "thinking", status: "running"))

        #expect(conn.turns.count == 2)
        #expect(conn.turns[1].isInProgress)
        #expect(conn.turns[1].text == "thinking")

        // The open turn re-sent with more text and a first item.
        let items = "[{\"type\":\"tool\",\"id\":\"item-0\",\"name\":\"Bash\",\"status\":\"completed\",\"input\":null,\"output\":\"ok\"}]"
        conn.handle(event: "turn", data: frame(id: "turn-2", text: "thinking\\nmore", status: "running", items: items))
        #expect(conn.turns.count == 2, "same id replaces, never appends")
        #expect(conn.turns[1].text == "thinking\nmore")
        #expect(conn.turns[1].items.count == 1)
        #expect(conn.turns[1].isInProgress)

        // Finalization flips the status under the same id.
        conn.handle(event: "turn", data: frame(id: "turn-2", text: "thinking\\nmore", status: "completed", items: items))
        #expect(conn.turns.count == 2)
        #expect(!conn.turns[1].isInProgress)
        #expect(conn.turns.map(\.id) == ["turn-1", "turn-2"])
    }

    @Test("running turns can finalize as failed or interrupted")
    func terminalStatusFlips() {
        let conn = connection()
        conn.handle(event: "turn", data: frame(id: "turn-1", text: "a", status: "running"))
        conn.handle(event: "turn", data: frame(id: "turn-1", text: "a", status: "interrupted"))
        #expect(conn.turns[0].status == .interrupted)

        conn.handle(event: "turn", data: frame(id: "turn-2", text: "b", status: "running"))
        conn.handle(event: "turn", data: frame(id: "turn-2", text: "b", status: "failed"))
        #expect(conn.turns[1].status == .failed)
        #expect(conn.turns.allSatisfy { !$0.isInProgress })
    }

    @Test("frames sort by sequence however they arrive")
    func framesSortBySequence() {
        let conn = connection()
        // Replay serves the open turn after the finalized thread; a user turn
        // committed mid-turn can also arrive out of id order.
        conn.handle(event: "turn", data: frame(id: "turn-3", role: "user", text: "hey", status: "completed"))
        conn.handle(event: "turn", data: frame(id: "turn-2", text: "grinding", status: "running"))
        #expect(conn.turns.map(\.id) == ["turn-2", "turn-3"])
    }

    @Test("unparseable ids keep a deterministic order")
    func unparseableIdsAreDeterministic() {
        let conn = connection()
        // Both fall to the `.max` sentinel sequence; id breaks the tie.
        conn.handle(event: "turn", data: frame(id: "weird-b", text: "b", status: "completed"))
        conn.handle(event: "turn", data: frame(id: "weird-a", text: "a", status: "completed"))
        conn.handle(event: "turn", data: frame(id: "turn-7", text: "real", status: "completed"))
        #expect(conn.turns.map(\.id) == ["turn-7", "weird-a", "weird-b"])
    }

    @Test("non-turn events are ignored")
    func nonTurnEventsIgnored() {
        let conn = connection()
        conn.handle(event: "ping", data: frame(id: "turn-1", text: "x", status: "running"))
        #expect(conn.turns.isEmpty)
    }
}

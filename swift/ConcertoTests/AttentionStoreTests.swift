import Foundation
import Testing
@testable import LoopflowCore

@MainActor
@Suite("AttentionStore")
struct AttentionStoreTests {
    @Test("orders surfaced items ahead of viewed and oldest first within priority")
    func ordersByUrgency() {
        let store = AttentionStore()
        let now = Date()
        store.setAll([
            AttentionItem(
                id: "1",
                waveId: "wave",
                runId: nil,
                kind: .algedonic,
                status: .viewed,
                title: "viewed",
                summary: "",
                context: .raw("{}"),
                surfacedAt: now.addingTimeInterval(-10)
            ),
            AttentionItem(
                id: "2",
                waveId: "wave",
                runId: nil,
                kind: .interactiveStep,
                status: .surfaced,
                title: "older review",
                summary: "",
                context: .raw("{}"),
                surfacedAt: now.addingTimeInterval(-100)
            ),
            AttentionItem(
                id: "3",
                waveId: "wave",
                runId: nil,
                kind: .interactiveStep,
                status: .surfaced,
                title: "newer review",
                summary: "",
                context: .raw("{}"),
                surfacedAt: now.addingTimeInterval(-20)
            ),
        ])

        #expect(store.ordered.map(\.id) == ["2", "3", "1"])
    }

    @Test("parseAttentionFromJSON decodes typed contexts by payload shape")
    func parsesAttention() throws {
        let json: [String: Any] = [
            "id": "attn-1",
            "wave_id": "wave-1",
            "run_id": "run-1",
            "kind": "algedonic",
            "status": "surfaced",
            "title": "Queue blocked",
            "summary": "Needs help",
            "context": [
                "reason": "rebase_conflict",
                "conflict_files": ["src/lib.rs"],
                "error": "merge failed",
            ],
            "surfaced_at": ISO8601DateFormatter().string(from: now),
        ]
        let item = WaveService.parseAttentionFromJSON(json)
        #expect(item?.kind == .algedonic)
        if case .queueFailure(let context) = item?.context {
            #expect(context.conflictFiles == ["src/lib.rs"])
            #expect(context.error == "merge failed")
        } else {
            Issue.record("Expected queue failure context")
        }
    }

    private var now: Date { Date(timeIntervalSince1970: 1_700_000_000) }
}

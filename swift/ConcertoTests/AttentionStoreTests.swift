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
                title: "older escalation",
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
                title: "newer escalation",
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
            Issue.record("Expected algedonic context")
        }
    }

    @Test("context decodes interactive from kind")
    func parsesInteractiveContext() {
        let context: [String: Any] = [
            "step": "code/design",
            "terminal_session_id": "ts-123",
            "design_path": "scratch/my-branch.md",
        ]
        let parsed = AttentionItem.context(kind: .interactive, json: context)
        if case .interactive(let ctx) = parsed {
            #expect(ctx.step == "code/design")
            #expect(ctx.terminalSessionId == "ts-123")
            #expect(ctx.designPath == "scratch/my-branch.md")
        } else {
            Issue.record("Expected interactive context, got \(parsed)")
        }
    }

    @Test("context decodes algedonic from kind")
    func parsesAlgedonicContext() {
        let context: [String: Any] = [
            "step": "implement",
            "error": "agent crashed",
        ]
        let parsed = AttentionItem.context(kind: .algedonic, json: context)
        if case .algedonic(let ctx) = parsed {
            #expect(ctx.step == "implement")
            #expect(ctx.error == "agent crashed")
        } else {
            Issue.record("Expected algedonic context, got \(parsed)")
        }
    }

    @Test("legacy kind strings map to collapsed kinds")
    func legacyKindMapping() {
        #expect(AttentionKind(rawValue: "design_review") == .interactive)
        #expect(AttentionKind(rawValue: "code_review") == .interactive)
        #expect(AttentionKind(rawValue: "calibration") == .interactive)
        #expect(AttentionKind(rawValue: "queue_failure") == .algedonic)
        #expect(AttentionKind(rawValue: "step_failure") == .algedonic)
        #expect(AttentionKind(rawValue: "interactive") == .interactive)
        #expect(AttentionKind(rawValue: "algedonic") == .algedonic)
    }

    @Test("parseAttentionFromJSON handles legacy kind strings")
    func parsesLegacyKinds() {
        let json: [String: Any] = [
            "id": "attn-legacy",
            "wave_id": "wave-1",
            "kind": "step_failure",
            "status": "surfaced",
            "title": "Step failed",
            "summary": "error",
            "context": ["error": "boom"] as [String: Any],
            "surfaced_at": ISO8601DateFormatter().string(from: now),
        ]
        let item = WaveService.parseAttentionFromJSON(json)
        #expect(item?.kind == .algedonic)
    }

    private var now: Date { Date(timeIntervalSince1970: 1_700_000_000) }
}

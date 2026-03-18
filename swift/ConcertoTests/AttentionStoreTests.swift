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
                kind: .codeReview,
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
                kind: .designReview,
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
                kind: .designReview,
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
            "kind": "queue_failure",
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
        #expect(item?.kind == .queueFailure)
        if case .queueFailure(let context) = item?.context {
            #expect(context.conflictFiles == ["src/lib.rs"])
            #expect(context.error == "merge failed")
        } else {
            Issue.record("Expected queue failure context")
        }
    }

    @Test("context decodes design review from step field")
    func parsesDesignReviewContext() {
        let context: [String: Any] = [
            "step": "code/design",
            "terminal_session_id": "ts-123",
            "design_path": "scratch/my-branch.md",
        ]
        let parsed = AttentionItem.context(json: context)
        if case .designReview(let ctx) = parsed {
            #expect(ctx.step == "code/design")
            #expect(ctx.terminalSessionId == "ts-123")
            #expect(ctx.designPath == "scratch/my-branch.md")
        } else {
            Issue.record("Expected design review context, got \(parsed)")
        }
    }

    @Test("context decodes calibration from chord step field")
    func parsesCalibrationContext() {
        let context: [String: Any] = [
            "step": "chord/review",
            "terminal_session_id": "ts-456",
            "design_path": "scratch/chord.md",
        ]
        let parsed = AttentionItem.context(json: context)
        if case .calibration(let ctx) = parsed {
            #expect(ctx.step == "chord/review")
            #expect(ctx.terminalSessionId == "ts-456")
            #expect(ctx.chordPath == "scratch/chord.md")
        } else {
            Issue.record("Expected calibration context, got \(parsed)")
        }
    }

    @Test("context decodes code review with step field present")
    func parsesCodeReviewWithStep() {
        let context: [String: Any] = [
            "step": "code/review",
            "pr_url": "https://github.com/org/repo/pull/1",
            "pr_number": 1,
            "pr_title": "Test PR",
            "branch": "main",
        ]
        let parsed = AttentionItem.context(json: context)
        if case .codeReview(let ctx) = parsed {
            #expect(ctx.prNumber == 1)
            #expect(ctx.prTitle == "Test PR")
        } else {
            Issue.record("Expected code review context, got \(parsed)")
        }
    }

    @Test("parseAttentionFromJSON decodes design review kind and context")
    func parsesDesignReviewAttention() {
        let json: [String: Any] = [
            "id": "attn-dr",
            "wave_id": "wave-1",
            "run_id": "run-1",
            "kind": "design_review",
            "status": "surfaced",
            "title": "test-wave needs design review",
            "summary": "Waiting for design review.",
            "context": [
                "step": "code/design",
                "terminal_session_id": "ts-789",
                "design_path": "scratch/test.md",
            ] as [String: Any],
            "surfaced_at": ISO8601DateFormatter().string(from: now),
        ]
        let item = WaveService.parseAttentionFromJSON(json)
        #expect(item?.kind == .designReview)
        if case .designReview(let ctx) = item?.context {
            #expect(ctx.step == "code/design")
            #expect(ctx.terminalSessionId == "ts-789")
        } else {
            Issue.record("Expected design review context")
        }
    }

    private var now: Date { Date(timeIntervalSince1970: 1_700_000_000) }
}

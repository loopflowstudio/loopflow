import Foundation
import Testing
@testable import Concerto
@testable import LoopflowCore

@MainActor
@Suite("Attention queue detail view")
struct AttentionQueueViewTests {
    @Test("review-design attention shows inline design preview")
    func reviewDesignShowsPreview() throws {
        let repoRoot = try makeRepoRoot()
        let scratch = repoRoot.appendingPathComponent("scratch")
        try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
        try """
        # Interactive checkpoints

        Surface every interactive step in the attention queue.
        Keep the session button visible.
        """.write(to: scratch.appendingPathComponent("feature.md"), atomically: true, encoding: .utf8)

        let item = AttentionItem(
            id: "attention-1",
            waveId: "wave-1",
            runId: "run-1",
            kind: .interactive,
            status: .surfaced,
            title: "Design review: wave-1",
            summary: "Surface every interactive step in the attention queue.",
            context: .interactive(
                InteractiveAttentionContext(
                    step: "review-design",
                    terminalSessionId: "terminal-1",
                    designPath: "scratch/feature.md",
                    mutationSummary: nil
                )
            ),
            surfacedAt: .now
        )
        guard case .interactive(let context) = item.context else {
            Issue.record("Expected interactive context")
            return
        }

        let preview = attentionDesignPreviewText(item: item, context: context, repoRoot: repoRoot)
        #expect(preview != nil)
        #expect(preview?.contains("Surface every interactive step in the attention queue.") == true)
    }

    @Test("wave-review attention shows mutation summary")
    func waveReviewShowsMutationSummary() throws {
        let item = AttentionItem(
            id: "attention-2",
            waveId: "wave-1",
            runId: "run-1",
            kind: .interactive,
            status: .surfaced,
            title: "Wave review: wave-1",
            summary: "Rebalance the PM wave.",
            context: .interactive(
                InteractiveAttentionContext(
                    step: "wave/review",
                    terminalSessionId: "terminal-1",
                    designPath: nil,
                    mutationSummary: "- Rebalance the PM wave.\n- Close stale roadmap items."
                )
            ),
            surfacedAt: .now
        )
        guard case .interactive(let context) = item.context else {
            Issue.record("Expected interactive context")
            return
        }

        let summary = attentionMutationSummary(context)
        #expect(summary != nil)
        #expect(summary?.contains("Rebalance the PM wave.") == true)
        #expect(summary?.contains("Close stale roadmap items.") == true)
    }

    private func makeRepoRoot() throws -> URL {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("attention-queue-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        return root
    }
}

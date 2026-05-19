import Foundation
import Testing
@testable import Concerto
@testable import LoopflowCore

@Suite("Wave Session View")
struct WaveSessionViewTests {
    @Test("Smart timestamps always show at turn boundaries and long gaps")
    func computesSmartTimestampLabels() {
        let start = Date(timeIntervalSince1970: 0)
        let userOne = SessionMessage(role: .user, content: "First", timestamp: start)
        let assistantOne = SessionMessage(role: .assistant, content: "Ack", timestamp: start.addingTimeInterval(20))
        let userTwo = SessionMessage(role: .user, content: "Next", timestamp: start.addingTimeInterval(40))
        let assistantTwo = SessionMessage(role: .assistant, content: "After gap", timestamp: start.addingTimeInterval(130))
        let assistantThree = SessionMessage(role: .assistant, content: "Long gap", timestamp: start.addingTimeInterval(4_000))

        let transcript: [TranscriptEntry] = [
            .message(userOne),
            .message(assistantOne),
            .message(userTwo),
            .message(assistantTwo),
            .message(assistantThree),
        ]

        let labels = messageTimestampLabels(for: transcript)

        #expect(labels[userOne.id] != nil)
        #expect(labels[assistantOne.id] == nil)
        #expect(labels[userTwo.id] != nil)
        #expect(labels[assistantTwo.id] == "1m ago")
        #expect(labels[assistantThree.id] != nil)
        #expect(labels[assistantThree.id]?.contains("ago") == false)
    }

    @Test("Context sources sort by token usage and include metadata")
    func contextSourcesSortAndAnnotate() {
        let snapshot = ContextSnapshot(
            sources: [
                "step": 1_200,
                "diff": 5_000,
                "repo_doc": 800,
                "system": 3_000,
            ],
            sourceCounts: [
                "diff": 8,
                "repo_doc": 2,
            ],
            documents: [
                DocumentEntry(path: "src/a.rs", source: "diff", tokens: 2_000),
                DocumentEntry(path: "README.md", source: "repo_doc", tokens: 800),
            ],
            budget: 75_000,
            total: 10_000,
            diffTier: "UnifiedDiff",
            stepName: "implement",
            directionNames: [],
            areaName: nil,
            waveName: nil,
            hasClipboard: false
        )

        let rows = contextSourceRows(snapshot: snapshot)
        #expect(rows.map(\.source) == ["diff", "system", "step", "repo_doc"])
        #expect(rows.first?.metadata == "unified (8 files)")
        #expect(rows.last?.metadata == "2 files")
    }

    @Test("Context document drill-down caps at ten entries")
    func contextDocumentSliceCapsToTopTen() {
        let documents = (0..<12).map { index in
            DocumentEntry(path: "src/file\(index).rs", source: "diff", tokens: UInt64(120 - index))
        }
        let snapshot = ContextSnapshot(
            sources: ["diff": 1200],
            sourceCounts: ["diff": 12],
            documents: documents,
            budget: 75_000,
            total: 1200,
            diffTier: "UnifiedDiff",
            stepName: nil,
            directionNames: [],
            areaName: nil,
            waveName: nil,
            hasClipboard: false
        )

        let slice = contextDocumentSlice(snapshot: snapshot, source: "diff")
        #expect(slice.visible.count == 10)
        #expect(slice.remainingCount == 2)
        #expect(slice.visible.first?.path == "src/file0.rs")
        #expect(slice.visible.last?.path == "src/file9.rs")
    }
}

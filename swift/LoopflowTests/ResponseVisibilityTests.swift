#if os(macOS)
@testable import Loopflow
import Testing
@testable import LoopflowMac

struct ResponseVisibilityTests {
    @Test func responseRequiresNewProviderTurnAndNewVisibleText() {
        let old = SessionPreview(providerThread: "thread", providerTurn: "old", observedAt: 99, source: "codex", text: "Old answer")
        let observation = ResponseVisibility(previous: old, before: "Question: element 74?", submittedAt: 100)
        let answer = SessionPreview(providerThread: "thread", providerTurn: "new", observedAt: 100, source: "codex", text: "Tungsten is the answer")
        #expect(!observation.matches(old, visible: "Old answer"))
        #expect(!observation.matches(answer, visible: "Working…"))
        #expect(observation.matches(answer, visible: "• Tungsten is\nthe answer"))
        let promptEcho = ResponseVisibility(previous: old, before: "Say Tungsten is the answer", submittedAt: 100)
        #expect(!promptEcho.matches(answer, visible: "Say Tungsten is the answer"))
        let stale = ResponseVisibility(previous: old, before: "", submittedAt: 101)
        #expect(!stale.matches(answer, visible: "Tungsten is the answer"))
    }
    @Test func nativeTranscriptResponseStillRequiresTheVisibleAssistantText() {
        let observation = ResponseVisibility(previous: nil, before: "Element 74?", submittedAt: 100)
        let answer = SessionPreview(providerThread: "thread", providerTurn: "new", observedAt: 100,
            source: "codex_transcript", text: "Tungsten")
        #expect(!observation.matches(answer, visible: "Working…"))
        #expect(observation.matches(answer, visible: "• Tungsten"))
    }

}
#endif

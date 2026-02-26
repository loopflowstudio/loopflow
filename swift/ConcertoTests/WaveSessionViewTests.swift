import Foundation
import Testing
@testable import Concerto
@testable import LoopflowCore

@Suite("Wave Session View")
struct WaveSessionViewTests {
    @Test("Assistant markdown splits into text and fenced code segments")
    func parsesTextAndCodeSegments() {
        let input = """
        Intro paragraph.

        ```swift
        let value = 42
        print(value)
        ```

        Outro line.
        """

        let segments = parseMessageSegments(input)

        #expect(segments.count == 3)

        if case .text(let intro) = segments[0] {
            #expect(intro.contains("Intro paragraph."))
        } else {
            Issue.record("Expected first segment to be text")
        }

        if case .code(let language, let code) = segments[1] {
            #expect(language == "swift")
            #expect(code.contains("let value = 42"))
            #expect(code.contains("print(value)"))
        } else {
            Issue.record("Expected second segment to be code")
        }

        if case .text(let outro) = segments[2] {
            #expect(outro.contains("Outro line."))
        } else {
            Issue.record("Expected third segment to be text")
        }
    }

    @Test("Unclosed fence still renders as a code segment")
    func parsesUnclosedFenceAsCode() {
        let input = """
        ```bash
        echo hello
        """

        let segments = parseMessageSegments(input)

        #expect(segments.count == 1)
        if case .code(let language, let code) = segments[0] {
            #expect(language == "bash")
            #expect(code == "echo hello")
        } else {
            Issue.record("Expected a code segment for unclosed fence")
        }
    }

    @Test("Plain text stays a single text segment")
    func parsesPlainTextAsSingleSegment() {
        let input = "No code fences here"

        let segments = parseMessageSegments(input)

        #expect(segments == [.text("No code fences here")])
    }

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
}

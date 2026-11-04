import Foundation
import Loopflow

/// Explicit offline demo. Input runs an editor; it is not a recording or a server.
@MainActor
final class FixtureClient: MobileClient {
    private var sessions: [String: FixtureTerminal] = [:]

    func workingTasks() async throws -> [WorkViewPlan.Content] {
        guard let url = Bundle.main.url(forResource: "task-reading", withExtension: "json") else {
            throw CocoaError(.fileNoSuchFile)
        }
        return try decodeMobileTasks(Data(contentsOf: url), requestID: "fixture-tasks")
    }

    func attach(sessionID: String) async throws -> any MobileAttachment {
        let terminal = sessions[sessionID] ?? FixtureTerminal()
        sessions[sessionID] = terminal
        return FixtureAttachment(sessionID: sessionID, terminal: terminal)
    }
}

@MainActor
final class FixtureAttachment: MobileAttachment {
    let sessionID: String
    let events: AsyncStream<MobileTerminalEvent>
    private let continuation: AsyncStream<MobileTerminalEvent>.Continuation
    let terminal: FixtureTerminal
    private var controlling = false
    private var closed = false
    private(set) var sizes: [(Int, Int)] = []

    init(sessionID: String, terminal: FixtureTerminal) {
        self.sessionID = sessionID
        self.terminal = terminal
        (events, continuation) = AsyncStream.makeStream()
        continuation.yield(.snapshot(Data(terminal.screen.utf8)))
        continuation.yield(.control(input: false, geometry: false))
    }

    func requestControl() throws {
        guard !closed else { throw CocoaError(.fileReadUnknown) }
        controlling = true
        continuation.yield(.control(input: true, geometry: true))
    }

    func send(_ bytes: Data) throws {
        guard controlling, !closed else { throw CocoaError(.fileWriteNoPermission) }
        terminal.accept(bytes)
        continuation.yield(.output(Data(terminal.screen.utf8)))
    }

    func resize(columns: Int, rows: Int) throws {
        guard controlling, !closed else { throw CocoaError(.fileWriteNoPermission) }
        sizes.append((columns, rows))
    }

    func loseControl() {
        controlling = false
        continuation.yield(.control(input: false, geometry: false))
    }

    func end() { continuation.yield(.ended) }
    func drop() { continuation.yield(.disconnected("Fixture connection dropped")) }

    func close() {
        closed = true
        controlling = false
        continuation.finish()
    }
}

/// Tiny deterministic terminal peer for the native input proof. Its prompt owns the draft.
/// Implements only the editor keys exercised here, never executes a command.
final class FixtureTerminal {
    private(set) var draft = ""
    private(set) var submissions: [String] = []
    private var cursor = 0
    private var pending: [UInt8] = []
    private var pasted = false
    private var asking = false
    private var result = "Type a request. Try permission, then y or n."

    var screen: String {
        let prompt = asking ? "Allow fixture action? [y/N] " : "> "
        // CUP restores the editor cursor after the full repaint, including multiline input.
        let prefix = String(draft.prefix(cursor))
        let lines = prefix.split(separator: "\n", omittingEmptySubsequences: false)
        let row = 5 + lines.count - 1
        let column = (lines.count == 1 ? prompt.count : 0) + (lines.last?.count ?? 0) + 1
        return "\u{1b}[?2004h\u{1b}[2J\u{1b}[H\u{1b}[1mLoopflow · fixture Session\u{1b}[0m\r\n"
            + "Offline input harness · no commands execute\r\n\(result)\r\n\r\n"
            + prompt + draft.replacingOccurrences(of: "\n", with: "\r\n")
            + "\u{1b}[\(row);\(column)H"
    }

    func accept(_ data: Data) {
        pending += data
        let controls: [([UInt8], () -> Void)] = [
            (Array("\u{1b}[200~".utf8), { self.pasted = true }),
            (Array("\u{1b}[201~".utf8), { self.pasted = false }),
            (Array("\u{1b}[D".utf8), { self.cursor = max(0, self.cursor - 1) }),
            (Array("\u{1b}[C".utf8), { self.cursor = min(self.draft.count, self.cursor + 1) }),
            (Array("\u{1b}[H".utf8), { self.cursor = 0 }),
            (Array("\u{1b}[F".utf8), { self.cursor = self.draft.count }),
        ]
        while let byte = pending.first {
            if byte == 27 {
                if let (sequence, action) = controls.first(where: { pending.starts(with: $0.0) }) {
                    pending.removeFirst(sequence.count)
                    action()
                    continue
                }
                if controls.contains(where: { $0.0.starts(with: pending) }) { return }
                pending.removeFirst()
                continue
            }
            if byte == 127 || byte == 8 {
                if cursor > 0 {
                    draft.remove(at: draft.index(draft.startIndex, offsetBy: cursor - 1))
                    cursor -= 1
                }
                pending.removeFirst()
            } else if byte == 13 && !pasted {
                submissions.append(draft)
                if asking {
                    result = draft.lowercased() == "y" ? "Fixture action confirmed." : "Fixture action canceled."
                    asking = false
                } else if draft == "permission" {
                    asking = true
                    result = "A fixture permission question is open."
                } else {
                    result = "Received \(draft.count) characters."
                }
                draft = ""
                cursor = 0
                pending.removeFirst()
            } else if byte == 3 {
                asking = false
                result = "Fixture interrupted."
                pending.removeFirst()
            } else {
                let count = byte < 0x80 ? 1 : byte < 0xE0 ? 2 : byte < 0xF0 ? 3 : 4
                guard pending.count >= count else { return }
                let text = String(decoding: pending.prefix(count), as: UTF8.self)
                    .replacingOccurrences(of: "\r", with: "\n")
                draft.insert(contentsOf: text, at: draft.index(draft.startIndex, offsetBy: cursor))
                cursor += text.count
                pending.removeFirst(count)
            }
        }
    }
}

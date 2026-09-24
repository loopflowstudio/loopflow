import Foundation
import Loopflow

/// Loaded display evidence; history and live retain independent order and precedence.
struct TaskWatchOutput: Identifiable {
    struct ID: Hashable {
        let run: String
        let source: OutputSource
    }

    private(set) var source: TaskOutputSource
    private struct ObservedRecord {
        var value: OutputRecord
        var position: TaskWatchOutputPosition
        var hasLiveRevision: Bool
        var lastLiveObservation: Int?
        var retainedAt: Int
        var payloadBytes: Int
    }

    private var records: [String: ObservedRecord] = [:]
    private(set) var historyHasMore = false
    private(set) var liveHasMore = false

    var id: ID { ID(run: source.runId, source: source.source) }

    init(source: TaskOutputSource) {
        self.source = source
        self.source.records = []
    }

    var retainedRecords: [(id: String, observation: Int, bytes: Int)] {
        records.map { id, record in
            (id, record.retainedAt, record.payloadBytes)
        }
    }

    mutating func removeRecords(_ ids: Set<String>) {
        for id in ids { records.removeValue(forKey: id) }
    }

    mutating func restartHistory() {
        records.removeAll()
        historyHasMore = false
    }

    mutating func merge(_ page: TaskOutputSource, history: Bool, observation: inout Int) {
        source = page
        source.records = []
        if history { historyHasMore = page.hasMore } else { liveHasMore = page.hasMore }
        for record in page.records {
            let id = record.sourceItemId
            guard var observed = records[id] else {
                observation += 1
                records[id] = ObservedRecord(value: record,
                    position: .init(history: history, offset: observation),
                    hasLiveRevision: !history, lastLiveObservation: history ? nil : observation,
                    retainedAt: observation, payloadBytes: record.payloadBytes)
                continue
            }
            if history {
                if !observed.position.history {
                    observation += 1
                    observed.position = .init(history: true, offset: observation)
                    observed.retainedAt = observation
                }
                if !observed.hasLiveRevision && observed.value.revision != record.revision {
                    observation += 1
                    observed.retainedAt = observation
                    observed.value = record
                    observed.payloadBytes = record.payloadBytes
                }
            } else {
                if observed.value.revision != record.revision {
                    observation += 1
                    observed.lastLiveObservation = observation
                    observed.retainedAt = observation
                    observed.payloadBytes = record.payloadBytes
                }
                observed.hasLiveRevision = true
                observed.value = record
            }
            records[id] = observed
        }
    }

    var rows: [TaskWatchOutputRow] {
        var result: [TaskWatchOutputRow] = []
        var positions: [TaskWatchOutputRow.ID: Int] = [:]
        var textRun: TaskWatchOutputRow.ID?
        var nativeCalls: [String: (turn: String, item: ConversationItem)] = [:]
        var previousPosition: TaskWatchOutputPosition?
        var lastLiveObservation: Int?
        func put(_ value: TaskWatchOutputRow, append: Bool = false) {
            var row = value
            row.lastLiveObservation = lastLiveObservation
            if let index = positions[row.id] {
                row.position = result[index].position
                row.lastLiveObservation = [row.lastLiveObservation, result[index].lastLiveObservation].compactMap { $0 }.max()
                if append { row.text = result[index].text + row.text }
                result[index] = row
            } else {
                positions[row.id] = result.count
                result.append(row)
            }
        }
        for observed in records.values.sorted(by: { $0.position < $1.position }) {
            let record = observed.value
            let id = record.sourceItemId
            let position = observed.position
            lastLiveObservation = observed.lastLiveObservation
            if !position.immediatelyFollows(previousPosition) { textRun = nil }
            defer { previousPosition = position }
            // Combine adjacent deltas, never move prose across an intervening
            // tool or message merely because both belong to the same turn.
            switch record.event {
            case .textDelta, .reasoningDelta: break
            default: textRun = nil
            }
            switch record.event {
            case let .itemStarted(turn, item), let .itemCompleted(turn, item):
                // Native JSONL results carry a call ID, but omit the original
                // name/input. Claude also puts the result in a different message.
                if source.source == .claude || source.source == .codex,
                   case let .tool(id, name, status, _, output) = item {
                    if name == "tool_result", let call = nativeCalls[id],
                       case let .tool(_, callName, _, input, _) = call.item {
                        put(TaskWatchOutputRow(position: position, turn: call.turn, item: .tool(
                            id: id, name: callName, status: status, input: input, output: output
                        )))
                        continue
                    }
                    if name != "tool_result" { nativeCalls[id] = (turn, item) }
                    if name == "tool_result" {
                        var row = TaskWatchOutputRow(position: position, turn: turn, item: item)
                        row.title = "Tool result · \(status.rawValue) (earlier call not loaded)"
                        put(row)
                        continue
                    }
                }
                put(TaskWatchOutputRow(position: position, turn: turn, item: item))
            case let .itemUpdated(turn, item, delta):
                switch delta {
                case let .output(text), let .planText(text):
                    let key = TaskWatchOutputRow.ID(turn: turn, item: item, kind: .item)
                    if let index = positions[key] {
                        result[index].text += text
                        result[index].lastLiveObservation = [result[index].lastLiveObservation, lastLiveObservation].compactMap { $0 }.max()
                    } else {
                        put(TaskWatchOutputRow(position: position, id: key, title: "Tool output (earlier context not loaded)", text: text, code: true))
                    }
                }
            case let .textDelta(turn, text):
                let key = textRun.flatMap { $0.turn == turn && $0.kind == .text ? $0 : nil }
                    ?? .init(turn: turn, item: id, kind: .text)
                put(TaskWatchOutputRow(position: position, id: key, title: "Streaming text", text: text), append: true)
                textRun = key
            case let .reasoningDelta(turn, text):
                let key = textRun.flatMap { $0.turn == turn && $0.kind == .reasoning ? $0 : nil }
                    ?? .init(turn: turn, item: id, kind: .reasoning)
                put(TaskWatchOutputRow(position: position, id: key, title: "Reasoning", text: text), append: true)
                textRun = key
            case let .error(code, message, _):
                put(TaskWatchOutputRow(position: position, id: .init(turn: "", item: id, kind: .event), title: "Error · \(code)", text: message))
            case let .turnCompleted(turn, status):
                put(TaskWatchOutputRow(position: position, id: .init(turn: turn, item: id, kind: .event), title: "Turn \(status.rawValue)", text: ""))
            case let .diffUpdated(turn, diff):
                put(TaskWatchOutputRow(position: position, id: .init(turn: turn, item: "", kind: .diff), title: "Changes", text: diff, code: true))
            case let .statusChanged(status):
                put(TaskWatchOutputRow(position: position, id: .init(turn: "", item: id, kind: .event), title: status, text: ""))
            case .turnStarted, .usageCheckpoint, .suggestedActions:
                break
            }
        }
        return result.filter { !$0.title.isEmpty || !$0.text.isEmpty }
    }
}

private extension OutputRecord {
    /// Account for payloads, including invisible evidence and tool inputs. The
    /// separate record limit bounds per-record container overhead.
    var payloadBytes: Int {
        func encodedSize(_ value: some Encodable) -> Int {
            // Decoded JSON should encode; an unencodable value cannot fit the window.
            (try? JSONEncoder().encode(value).count) ?? Int(Int32.max)
        }
        let eventBytes: Int
        switch event {
        case let .itemStarted(turn, item), let .itemCompleted(turn, item):
            eventBytes = turn.utf8.count + encodedSize(item)
        case let .itemUpdated(turn, item, delta):
            switch delta {
            case let .output(text), let .planText(text):
                eventBytes = turn.utf8.count + item.utf8.count + text.utf8.count
            }
        case let .textDelta(turn, text), let .reasoningDelta(turn, text), let .diffUpdated(turn, text):
            eventBytes = turn.utf8.count + text.utf8.count
        case let .turnStarted(turn): eventBytes = turn.utf8.count
        case let .turnCompleted(turn, status): eventBytes = turn.utf8.count + status.rawValue.utf8.count
        case let .statusChanged(status): eventBytes = status.utf8.count
        case let .error(code, message, evidence):
            eventBytes = code.utf8.count + message.utf8.count + encodedSize(evidence)
        case let .usageCheckpoint(turn, usage, _):
            eventBytes = turn.utf8.count + encodedSize(usage)
        case let .suggestedActions(turn, actions):
            eventBytes = turn.utf8.count + encodedSize(actions)
        }
        return sourceItemId.utf8.count + revision.utf8.count + eventBytes
    }
}

struct TaskWatchOutputRow: Identifiable {
    struct ID: Hashable {
        enum Kind: Hashable { case item, text, reasoning, diff, event }
        let turn: String
        let item: String
        let kind: Kind
    }
    let id: ID
    var position: TaskWatchOutputPosition
    var lastLiveObservation: Int?
    var title: String
    var text: String
    var code = false

    init(position: TaskWatchOutputPosition, id: ID, title: String, text: String, code: Bool = false) {
        self.position = position
        self.id = id
        self.title = title
        self.text = text
        self.code = code
    }

    init(position: TaskWatchOutputPosition, turn: String, item: ConversationItem) {
        self.position = position
        id = ID(turn: turn, item: item.id, kind: .item)
        switch item {
        case let .message(_, text, phase):
            title = phase?.capitalized ?? "Message"
            self.text = text
        case let .thought(_, text):
            title = item.isVisibleInConversation ? "Thought" : ""
            self.text = item.isVisibleInConversation ? text : ""
        case let .command(_, command, cwd, status, output, exitCode, _):
            title = "Command · \(status.rawValue)" + (exitCode.map { " · exit \($0)" } ?? "")
            text = command.joined(separator: " ") + "\n" + cwd + "\n" + (output ?? "")
            code = true
        case let .tool(_, name, status, input, output):
            title = "\(name) · \(status.rawValue)"
            text = [input?.displayString, output].compactMap { $0 }.joined(separator: "\n")
            code = true
        case let .file(_, changes, status):
            title = "Files · \(status.rawValue)"
            text = changes.map { [$0.path, $0.diff].compactMap { $0 }.joined(separator: "\n") }.joined(separator: "\n\n")
            code = true
        case let .unknown(_, type):
            title = "Unsupported item · \(type)"
            text = ""
        }
    }
}

/// History pages precede arrivals; neither lane asserts cross-provider chronology.
struct TaskWatchOutputPosition: Comparable, Hashable {
    let history: Bool
    let offset: Int

    static func < (lhs: Self, rhs: Self) -> Bool {
        lhs.history == rhs.history ? lhs.offset < rhs.offset : lhs.history
    }

    func immediatelyFollows(_ previous: Self?) -> Bool {
        previous?.history == history && previous?.offset == offset - 1
    }
}

struct TaskWatchOutputRowReference: Hashable {
    let source: TaskWatchOutput.ID
    let row: TaskWatchOutputRow.ID
}

/// A contiguous stretch of one source in the observed feed, derived from loaded rows.
struct TaskWatchOutputGroup: Identifiable {
    let source: TaskOutputSource
    var rows: [TaskWatchOutputRow]
    var id: TaskWatchOutputPosition { rows[0].position }
    var sourceId: TaskWatchOutput.ID { .init(run: source.runId, source: source.source) }
    var history: Bool { rows[0].position.history }
}

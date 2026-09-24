import Foundation
import Loopflow

/// Loaded display evidence; history and live retain independent order and precedence.
struct TaskWatchOutput: Identifiable {
    struct ID: Hashable {
        let run: String
        let source: OutputSource
    }

    private(set) var source: TaskOutputSource
    private var records: [String: OutputRecord] = [:]
    private var historyOrder: [String] = []
    private var liveOrder: [String] = []
    private var liveIds: Set<String> = []
    private var historyIds: Set<String> = []
    private(set) var historyHasMore = false
    private(set) var liveHasMore = false

    var id: ID { ID(run: source.runId, source: source.source) }

    init(source: TaskOutputSource) { self.source = source }

    func containsRevision(_ record: OutputRecord) -> Bool {
        records[record.sourceItemId]?.revision == record.revision
    }

    mutating func merge(_ page: TaskOutputSource, history: Bool) {
        source = page
        if history { historyHasMore = page.hasMore } else { liveHasMore = page.hasMore }
        for record in page.records {
            let id = record.sourceItemId
            if history {
                if historyIds.insert(id).inserted { historyOrder.append(id) }
                if !liveIds.contains(id) { records[id] = record }
            } else {
                if liveIds.insert(id).inserted { liveOrder.append(id) }
                records[id] = record
            }
        }
    }

    var rows: [TaskWatchOutputRow] {
        var result: [TaskWatchOutputRow] = []
        var positions: [TaskWatchOutputRow.ID: Int] = [:]
        var textRun: TaskWatchOutputRow.ID?
        var nativeCalls: [String: (turn: String, item: ConversationItem)] = [:]
        func put(_ row: TaskWatchOutputRow, append: Bool = false) {
            if let index = positions[row.id] {
                if append { result[index].text += row.text } else { result[index] = row }
            } else {
                positions[row.id] = result.count
                result.append(row)
            }
        }
        for id in historyOrder + liveOrder.filter({ !historyIds.contains($0) }) {
            guard let record = records[id] else { continue }
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
                        put(TaskWatchOutputRow(turn: call.turn, item: .tool(
                            id: id, name: callName, status: status, input: input, output: output
                        )))
                        continue
                    }
                    if name != "tool_result" { nativeCalls[id] = (turn, item) }
                }
                put(TaskWatchOutputRow(turn: turn, item: item))
            case let .itemUpdated(turn, item, delta):
                switch delta {
                case let .output(text), let .planText(text):
                    let key = TaskWatchOutputRow.ID(turn: turn, item: item, kind: .item)
                    if let index = positions[key] {
                        result[index].text += text
                    } else {
                        put(TaskWatchOutputRow(id: key, title: "Tool output (earlier context not loaded)", text: text, code: true))
                    }
                }
            case let .textDelta(turn, text):
                let key = textRun.flatMap { $0.turn == turn && $0.kind == .text ? $0 : nil }
                    ?? .init(turn: turn, item: id, kind: .text)
                put(TaskWatchOutputRow(id: key, title: "Streaming text", text: text), append: true)
                textRun = key
            case let .reasoningDelta(turn, text):
                let key = textRun.flatMap { $0.turn == turn && $0.kind == .reasoning ? $0 : nil }
                    ?? .init(turn: turn, item: id, kind: .reasoning)
                put(TaskWatchOutputRow(id: key, title: "Reasoning", text: text), append: true)
                textRun = key
            case let .error(code, message, _):
                put(TaskWatchOutputRow(id: .init(turn: "", item: id, kind: .event), title: "Error · \(code)", text: message))
            case let .turnCompleted(turn, status):
                put(TaskWatchOutputRow(id: .init(turn: turn, item: id, kind: .event), title: "Turn \(status.rawValue)", text: ""))
            case let .diffUpdated(turn, diff):
                put(TaskWatchOutputRow(id: .init(turn: turn, item: "", kind: .diff), title: "Changes", text: diff, code: true))
            case let .statusChanged(status):
                put(TaskWatchOutputRow(id: .init(turn: "", item: id, kind: .event), title: status, text: ""))
            case .turnStarted, .usageCheckpoint, .suggestedActions:
                break
            }
        }
        return result.filter { !$0.title.isEmpty || !$0.text.isEmpty }
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
    var title: String
    var text: String
    var code = false

    init(id: ID, title: String, text: String, code: Bool = false) {
        self.id = id
        self.title = title
        self.text = text
        self.code = code
    }

    init(turn: String, item: ConversationItem) {
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

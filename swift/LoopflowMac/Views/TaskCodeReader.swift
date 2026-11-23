#if os(macOS)
import Loopflow
import SwiftUI

struct TaskCodeReader: View {
    let path: String
    let text: String
    var location: FileMatchLocation?
    var revision: String?
    let ready: () -> Void
    @Environment(\.palette) private var palette
    @Environment(\.theme) private var theme
    @State private var lines: [CodeLine]?

    var body: some View {
        Group {
            if let lines {
                ScrollViewReader { proxy in
                    ScrollView([.horizontal, .vertical]) {
                        LazyVStack(alignment: .leading, spacing: 0) {
                            ForEach(lines.indices, id: \.self) { index in
                                HStack(alignment: .firstTextBaseline, spacing: Spacing.md) {
                                    Text(verbatim: String(lines.count)).hidden()
                                        .overlay(alignment: .trailing) { Text(verbatim: String(index + 1)) }
                                        .foregroundStyle(palette.textSecondary)
                                    Text(painted(lines[index], number: index + 1))
                                        .textSelection(.enabled)
                                }
                            }
                        }
                        .font(Typography.mono)
                        .padding(Spacing.sm)
                    }
                    .defaultScrollAnchor(.topLeading, for: .alignment)
                    .task(id: location) {
                        if let hit = location?.hit { proxy.scrollTo(hit.line - 1, anchor: UnitPoint(x: 0, y: 0.5)) }
                    }
                }
                .onAppear(perform: ready)
                .accessibilityIdentifier("files-code-reader")
            } else {
                ProgressView()
            }
        }
        .task(id: text) {
            let result = await Task.detached(priority: .userInitiated) { [text, path] in
                CodeLine.read(text, grammar: Grammar.named(path))
            }.value
            guard !Task.isCancelled else { return }
            lines = result
        }
    }

    private func painted(_ line: CodeLine, number: Int) -> AttributedString {
        var value = AttributedString(line.text)
        value.paint(line.highlights, inks: .of(theme))
        if let hit = location?.hit, hit.line == number, hit.revision == revision,
           let range = hit.range(in: line.text), let attributed = Range(range, in: value) {
            value[attributed].backgroundColor = palette.keyboardFocus.opacity(0.3)
        }
        return value
    }
}

extension TaskGrepHit {
    func range(in text: String) -> NSRange? {
        let bytes = text.utf8
        guard start >= 0, end >= start,
              let from = bytes.index(bytes.startIndex, offsetBy: start, limitedBy: bytes.endIndex),
              let to = bytes.index(from, offsetBy: end - start, limitedBy: bytes.endIndex),
              let lower = from.samePosition(in: text.unicodeScalars),
              let upper = to.samePosition(in: text.unicodeScalars) else { return nil }
        return NSRange(lower..<upper, in: text)
    }
}
#endif

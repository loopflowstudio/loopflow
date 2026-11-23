#if os(macOS)
import Loopflow
import SwiftUI

struct TaskCodeReader: View {
    let path: String
    let text: String
    let ready: () -> Void
    @Environment(\.palette) private var palette
    @Environment(\.theme) private var theme
    @State private var lines: [CodeLine]?

    var body: some View {
        Group {
            if let lines {
                ScrollView([.horizontal, .vertical]) {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(lines.indices, id: \.self) { index in
                            HStack(alignment: .firstTextBaseline, spacing: Spacing.md) {
                                Text(verbatim: String(lines.count)).hidden()
                                    .overlay(alignment: .trailing) { Text(verbatim: String(index + 1)) }
                                    .foregroundStyle(palette.textSecondary)
                                Text(painted(lines[index]))
                                    .textSelection(.enabled)
                            }
                        }
                    }
                    .font(Typography.mono)
                    .padding(Spacing.sm)
                }
                .defaultScrollAnchor(.topLeading, for: .alignment)
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

    private func painted(_ line: CodeLine) -> AttributedString {
        var value = AttributedString(line.text)
        value.paint(line.highlights, inks: .of(theme))
        return value
    }
}
#endif

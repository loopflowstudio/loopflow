import Loopflow
import SwiftUI

/// The one status mark: a flat dot and a sans phrase in the tone's ink, with
/// the next step beside it as a button where one exists. Without a phrase the
/// dot stands beside a name that already says what it marks.
struct StatusMark<Action: View>: View {
    let phrase: String?
    let tone: WorkTone
    /// The tooltip, and what VoiceOver reads when there is no phrase.
    var detail: String?
    @ViewBuilder var action: Action
    @Environment(\.palette) private var palette

    var body: some View {
        let ink = tone.ink(in: palette)
        HStack(spacing: Spacing.sm) {
            HStack(spacing: Spacing.sm) {
                Circle().fill(ink).frame(width: Spacing.sm, height: Spacing.sm)
                if let phrase {
                    Text(verbatim: phrase)
                        .font(Typography.text)
                        .foregroundStyle(ink)
                        .lineLimit(1)
                        .truncationMode(.tail)
                }
            }
            .help(Text(verbatim: detail ?? phrase ?? ""))
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(Text(verbatim: phrase ?? detail ?? ""))
            .accessibilityHidden(phrase == nil && detail == nil)
            action
        }
    }
}

extension StatusMark where Action == EmptyView {
    init(_ phrase: String?, tone: WorkTone, detail: String? = nil) {
        self.init(phrase: phrase, tone: tone, detail: detail) { EmptyView() }
    }

    /// A program's report: its text, literal; the tooltip lists every bounded record.
    init?(status: ProgramStatusRecords) {
        guard let report = status.summary else { return nil }
        self.init(report.displayText, tone: report.state.workTone, detail: status.records.map { record in
            let source = ProgramStatusReport.displaySafe(record.id ?? "Program")
            let app = status.app(for: record).map { " (\(ProgramStatusReport.displaySafe($0)))" } ?? ""
            let title = record.title.map { " · \(ProgramStatusReport.displaySafe($0))" } ?? ""
            return "\(source)\(app): \(record.displayText)\(title)"
        }.joined(separator: "\n"))
    }

    /// A Wave's or Task's operating state, as a dot beside its name.
    init(lens: WaveLens) {
        self.init(nil, tone: lens.color.tone, detail: lens.reason)
    }
}

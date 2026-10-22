import Loopflow
import SwiftUI

/// Session edits retain the navigation-owned draft without adding a second header.
struct SessionActionSheet: View {
    @Bindable var model: WorkModel
    @Environment(\.palette) private var palette
    var body: some View {
        if model.navigation.binding != nil {
            bindingForm
        } else if let draft = model.navigation.renaming {
            VStack(alignment: .leading, spacing: 12) {
                Text("Rename Session").font(.headline)
                TextField("Session name", text: Binding(
                    get: { model.navigation.renaming?.text ?? draft.text },
                    set: { model.navigation.renaming?.text = $0 }))
                    .disabled(draft.submitting)
                    .onSubmit { Task { await model.commitSessionRename() } }
                    .accessibilityIdentifier("session-rename-field")
                if let error = draft.error { Text(error).foregroundStyle(palette.warning) }
                HStack {
                    Button("Cancel") { model.cancelSessionRename() }.keyboardShortcut(.cancelAction)
                    Button("Save") { Task { await model.commitSessionRename() } }.keyboardShortcut(.defaultAction)
                }.disabled(draft.submitting)
            }.padding(20).frame(width: 380)
        }
    }

    @ViewBuilder
    private var bindingForm: some View {
        if let draft = model.navigation.binding {
            VStack(alignment: .leading, spacing: 12) {
                Text("Bind “\(draft.title)” to a Task").font(Typography.textStrong)
                if let preview = draft.preview {
                    Text("\(preview.identifier) · \(preview.title)")
                    Text("This assignment is permanent. This Session cannot be moved to another Task or unbound.")
                    HStack {
                        Button("Choose another Task") { model.navigation.binding?.preview = nil }
                        Button("Bind permanently") { Task { await model.commitSessionBinding() } }
                            .accessibilityIdentifier("session-bind-confirm")
                    }
                    .disabled(draft.submitting)
                } else {
                    TextField("Task identifier or stable ID", text: Binding(
                        get: { model.navigation.binding?.selector ?? "" },
                        set: { model.navigation.binding?.selector = $0 }))
                        .disabled(draft.submitting)
                        .accessibilityIdentifier("session-bind-target")
                    Button("Review assignment") { Task { await model.previewSessionBinding() } }
                        .disabled(draft.submitting || draft.selector.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        .accessibilityIdentifier("session-bind-preview")
                }
                if let error = draft.error {
                    Text(error).foregroundStyle(palette.warning)
                        .accessibilityIdentifier("session-bind-error")
                }
                Button("Cancel") { model.cancelSessionBinding() }.disabled(draft.submitting)
            }
            .padding(16)
            .frame(width: 380)
            .lineLimit(nil)
            .fixedSize(horizontal: false, vertical: true)
        }
    }

}

/// Report text is always literal; the tooltip exposes the other bounded records.
struct ProgramStatusLabel: View {
    let status: ProgramStatusRecords
    @Environment(\.palette) private var palette

    var body: some View {
        if let report = status.summary {
            Text(verbatim: report.displayText)
                .foregroundStyle(report.state.workTone.ink(in: palette))
                .font(Typography.code(11))
                .lineLimit(1)
                .truncationMode(.tail)
                .frame(maxWidth: 340, alignment: .leading)
                .help(Text(verbatim: status.records.map { record in
                    let source = ProgramStatusReport.displaySafe(record.id ?? "Program")
                    let app = status.app(for: record).map { " (\(ProgramStatusReport.displaySafe($0)))" } ?? ""
                    let title = record.title.map { " · \(ProgramStatusReport.displaySafe($0))" } ?? ""
                    return "\(source)\(app): \(record.displayText)\(title)"
                }.joined(separator: "\n")))
        }
    }
}

import Loopflow
import SwiftUI

/// Explicit secondary Sessions keep their naming and binding actions beside their panes.
struct WorkSessionToolbar: View {
    @Bindable var model: WorkModel
    let crumb: WorkBreadcrumb?
    let onOpenSession: (SessionRecord) -> Void
    @Environment(\.palette) private var palette

    var body: some View {
        if let crumb, let session = crumb.session, !session.taskPrimary, session.primaryScope == nil {
            HStack(spacing: 8) {
                sessionCrumb(session, siblings: crumb.siblings.filter(model.isSessionVisible), isTask: crumb.taskWork != nil)
                Spacer(minLength: 0)
            }
            .font(Typography.text).padding(.horizontal, 14).padding(.vertical, 6)
            .background(palette.surfaceMuted)
            .accessibilityIdentifier("work-session-toolbar")
        }
    }

    @ViewBuilder
    private func sessionCrumb(_ session: SessionRecord, siblings: [SessionRecord], isTask: Bool) -> some View {
        if let draft = model.navigation.renaming, draft.sessionId == session.id {
            TextField("Session name", text: Binding(
                get: { model.navigation.renaming?.text ?? draft.text },
                set: { model.navigation.renaming?.text = $0 }
            ))
            .textFieldStyle(.plain)
            .frame(minWidth: 140, maxWidth: 260)
            .disabled(draft.submitting)
            .onSubmit { Task { await model.commitSessionRename() } }
            .onExitCommand { model.cancelSessionRename() }
            .accessibilityIdentifier("session-rename-field")
            Button("Save") { Task { await model.commitSessionRename() } }
                .disabled(draft.submitting)
                .accessibilityIdentifier("session-rename-save")
            Button("Cancel") { model.cancelSessionRename() }
                .disabled(draft.submitting)
            if let error = draft.error {
                Text(error)
                    .foregroundStyle(palette.waiting)
                    .lineLimit(2)
                    .accessibilityIdentifier("session-rename-error")
            }
        } else {
            if !isTask, siblings.count > 1 {
                Menu(session.title) {
                    ForEach(siblings) { record in
                        Button(record.title) { onOpenSession(record) }
                            .accessibilityIdentifier("breadcrumb-session-\(record.id)")
                    }
                }
                .fixedSize()
                .accessibilityIdentifier("breadcrumb-session")
            } else {
                Text(session.title)
                    .font(Typography.textStrong)
                    .accessibilityIdentifier("breadcrumb-session")
            }
            if session.titleSource == .unavailable {
                Text("name on its Machine")
                    .foregroundStyle(palette.textSecondary)
                    .help("This Session's canonical name lives on the Machine that runs it.")
            } else {
                Button {
                    model.beginSessionRename(session)
                } label: {
                    Image(systemName: "pencil")
                }
                .buttonStyle(.borderless)
                .help("Rename this Session")
                .accessibilityLabel("Rename Session")
                .accessibilityIdentifier("session-rename")
            }
            if session.work?.kind != .task || model.navigation.binding?.sessionId == session.id {
                Button("Bind to Task…") { model.beginSessionBinding(session) }
                    .buttonStyle(.borderless)
                    .accessibilityIdentifier("session-bind")
                    .popover(isPresented: Binding(
                        get: { model.navigation.binding?.sessionId == session.id },
                        set: { if !$0 { model.cancelSessionBinding() } }
                    )) { bindingForm }
            }
            if !isTask {
                membershipChip(session)
                    .help(membershipHelp(session.flowMembership))
                    .accessibilityIdentifier("session-flow-membership")
            }
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
                    Text(error).foregroundStyle(palette.waiting)
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

    /// Membership is a fact, so it reads as quiet mono text; blue stays with
    /// running work and loop regions.
    private func membershipChip(_ session: SessionRecord) -> some View {
        Text(session.flowMembership.label)
            .font(Typography.mono)
            .foregroundStyle(membershipColor(session.flowMembership))
            .lineLimit(1)
            .truncationMode(.tail)
            .layoutPriority(-1)
    }

    private func membershipColor(_ membership: SessionFlowMembership) -> Color {
        if case .step = membership { return palette.textSecondary }
        return palette.textTertiary
    }

    private func membershipHelp(_ membership: SessionFlowMembership) -> String {
        switch membership {
        case .step(_, _, _, _, _, .unknown):
            "This conversation retains Flow membership, but its relative position is unknown."
        case .step(_, _, _, _, _, .current):
            "This conversation is the Flow's current step."
        case .step(_, _, _, _, _, .earlier):
            "This conversation belonged to an earlier step of the Flow's current run."
        case .step(_, _, _, _, _, .past):
            "This conversation belonged to a Flow process that has since finished or been followed by a newer one."
        case .independent:
            "This conversation is not part of a Flow."
        case .unknown(let reason):
            reason
        }
    }
}

/// A toolbar action as a quiet glyph: no border, a hover tint, a 28pt target.
struct WorkGlyphButton: View {
    let systemImage: String
    let label: String
    let identifier: String
    let action: () -> Void
    @Environment(\.palette) private var palette
    @State private var hovering = false

    init(_ systemImage: String, label: String, identifier: String, action: @escaping () -> Void) {
        self.systemImage = systemImage
        self.label = label
        self.identifier = identifier
        self.action = action
    }

    var body: some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .font(.system(size: 13))
                .foregroundStyle(hovering ? palette.text : palette.textTertiary)
                .frame(width: 28, height: 28)
                .background(hovering ? palette.actionHover : Color.clear,
                            in: RoundedRectangle(cornerRadius: 6))
                .contentShape(Rectangle())
        }
        .buttonStyle(WorkPlainButtonStyle())
        .onHover { hovering = $0 }
        .help(label)
        .accessibilityLabel(label)
        .accessibilityIdentifier(identifier)
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

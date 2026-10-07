import Loopflow
import SwiftUI

/// The workspace's one toolbar: the Wave → Task → Session drill-down on the
/// left, the surface's controls on the right. It sits in the sidebar tone and
/// its bottom hairline continues the sidebar's right hairline, so frame and
/// navigation read as one inverted L. Ancestors navigate upward; the final
/// crumb names the exact Session, chooses among its siblings, and renames it
/// in place through the shared Session authority.
struct WorkBreadcrumbBar<Trailing: View>: View {
    @Bindable var model: WorkModel
    let crumb: WorkBreadcrumb?
    let onOpenSession: (SessionRecord) -> Void
    var onTaskDetails: (() -> Void)? = nil
    var programStatus: ProgramStatusRecords? = nil
    var programStatusError: String? = nil
    @ViewBuilder var trailing: Trailing
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: 10) {
            if let crumb { crumbs(crumb) }
            Spacer(minLength: 10)
            if let programStatus, programStatus.summary != nil {
                ProgramStatusLabel(status: programStatus)
                    .foregroundStyle(palette.textSecondary)
                    .accessibilityIdentifier("workspace-program-status")
            }
            if let programStatusError {
                Image(systemName: "exclamationmark.circle")
                    .help(programStatusError).accessibilityLabel(programStatusError)
            }
            trailing
            if model.navigation.content == .details {
                WorkGlyphButton(
                    "sidebar.right",
                    label: model.navigation.showsActivity ? "Hide Activity" : "Show Activity",
                    identifier: "work-toggle-activity"
                ) { model.navigation.showsActivity.toggle() }
            }
        }
        .fixedSize(horizontal: false, vertical: true)
        .font(Typography.text)
        .tint(palette.textSecondary)
        .padding(.horizontal, 14)
        .frame(height: TaskFileChrome.headerHeight)
        .background(palette.surfaceMuted)
        .overlay(alignment: .bottom) { Rectangle().fill(palette.border).frame(height: 1) }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("work-toolbar")
    }

    private func crumbs(_ crumb: WorkBreadcrumb) -> some View {
        HStack(spacing: 6) {
            if let work = crumb.waveWork {
                Button(crumb.wave?.roadmap.wave.name ?? "Wave \(work.id)") { model.select(work) }
                    .buttonStyle(.plain)
                    .foregroundStyle(palette.textSecondary)
                    .help(crumb.wave == nil ? "Wave name unavailable" : "Show Wave")
                    .disabled(crumb.wave == nil)
                    .accessibilityIdentifier("breadcrumb-wave")
            }
            if let task = crumb.task {
                if crumb.waveWork != nil { separator }
                // The Task overview shows its title once below; inside a
                // Session the ancestor carries the title to navigate upward.
                if let onTaskDetails {
                    Button(task.task.task.name, action: onTaskDetails)
                        .buttonStyle(.plain).foregroundStyle(palette.textSecondary)
                        .lineLimit(1).truncationMode(.tail).layoutPriority(-2)
                        .help("Task details and Flow")
                        .accessibilityIdentifier("breadcrumb-task")
                } else if crumb.session != nil {
                    // The ancestor's title gives way first when the bar is narrow.
                    Button(task.task.task.name) { model.select(task.id.work) }
                        .buttonStyle(.plain)
                        .foregroundStyle(palette.textSecondary)
                        .lineLimit(1)
                        .truncationMode(.tail)
                        .layoutPriority(-2)
                        .help(task.task.task.name)
                        .accessibilityIdentifier("breadcrumb-task")
                }
                if let url = task.task.reference.issueUrl {
                    Link(destination: url) {
                        Text(task.task.task.identifier)
                            .font(Typography.code(12))
                            .underline(color: palette.accentInk.opacity(0.35))
                    }
                    .foregroundStyle(palette.accentInk)
                        .accessibilityIdentifier("breadcrumb-issue")
                } else {
                    Text(task.task.task.identifier).font(Typography.code(12)).foregroundStyle(palette.textTertiary)
                }
            } else if let work = crumb.taskWork {
                if crumb.waveWork != nil { separator }
                Text("Task \(work.id)")
                    .font(Typography.code(12))
                    .foregroundStyle(palette.textSecondary)
                    .help("Task name and planning details unavailable")
                    .accessibilityIdentifier("breadcrumb-task")
            }
            if let session = crumb.session {
                if crumb.waveWork != nil || crumb.taskWork != nil { separator }
                sessionCrumb(session, siblings: crumb.siblings.filter(model.isSessionVisible))
            }
        }
        .lineLimit(1)
    }

    private var separator: some View {
        Text("/").foregroundStyle(palette.borderStrong)
    }

    @ViewBuilder
    private func sessionCrumb(_ session: SessionRecord, siblings: [SessionRecord]) -> some View {
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
                    .foregroundStyle(Color.statusWarning)
                    .lineLimit(2)
                    .accessibilityIdentifier("session-rename-error")
            }
        } else {
            if siblings.count > 1 {
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
                Text("name on its Home")
                    .foregroundStyle(palette.textSecondary)
                    .help("This Session's canonical name lives on the Home that runs it.")
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
            membershipChip(session)
                .help(membershipHelp(session.flowMembership))
                .accessibilityIdentifier("session-flow-membership")
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
                    Text(error).foregroundStyle(Color.statusWarning)
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
            "This conversation belonged to a Flow exec that has since finished or been followed by a newer one."
        case .independent:
            "This conversation is not part of a Flow."
        case .unknown(let reason):
            reason
        }
    }
}

extension WorkBreadcrumbBar where Trailing == EmptyView {
    init(model: WorkModel, crumb: WorkBreadcrumb?,
         onOpenSession: @escaping (SessionRecord) -> Void) {
        self.init(model: model, crumb: crumb, onOpenSession: onOpenSession) { EmptyView() }
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
                .foregroundStyle(hovering ? palette.accentInk : palette.textTertiary)
                .frame(width: 28, height: 28)
                .background(hovering ? palette.accentInk.opacity(0.08) : Color.clear,
                            in: RoundedRectangle(cornerRadius: 6))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .help(label)
        .accessibilityLabel(label)
        .accessibilityIdentifier(identifier)
    }
}

/// Report text is always literal; the tooltip exposes the other bounded records.
struct ProgramStatusLabel: View {
    let status: ProgramStatusRecords

    var body: some View {
        if let report = status.summary {
            Text(verbatim: report.displayText)
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

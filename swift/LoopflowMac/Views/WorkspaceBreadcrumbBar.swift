import Loopflow
import SwiftUI

/// Wave → Task → Session drill-down. Ancestors navigate upward; the final
/// crumb names the exact Session, chooses among its siblings, and renames it
/// in place through the shared Session authority.
struct WorkspaceBreadcrumbBar: View {
    @Bindable var model: PodiumModel
    let crumb: WorkspaceBreadcrumb
    let onOpenSession: (SessionRecord) -> Void
    let onMonitor: (String) -> Void
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: 6) {
            if let wave = crumb.wave {
                Button(wave.roadmap.wave.name) { model.select(wave.id.work) }
                    .buttonStyle(.link)
                    .accessibilityIdentifier("breadcrumb-wave")
            }
            if let task = crumb.task {
                separator
                Button(task.task.task.name) { model.select(task.id.work) }
                    .buttonStyle(.link)
                    .lineLimit(1)
                    .truncationMode(.tail)
                    .help(task.task.task.name)
                    .accessibilityIdentifier("breadcrumb-task")
                if let url = task.task.reference.issueUrl {
                    Link(task.task.task.identifier, destination: url)
                        .foregroundStyle(palette.textSecondary)
                        .accessibilityIdentifier("breadcrumb-issue")
                } else {
                    Text(task.task.task.identifier).foregroundStyle(palette.textSecondary)
                }
            }
            if let session = crumb.session {
                if crumb.wave != nil { separator }
                sessionCrumb(session)
            }
            Spacer(minLength: 0)
            if let task = crumb.task {
                if crumb.session == nil {
                    Menu("Sessions") {
                        ForEach(crumb.siblings) { record in
                            Button(record.title) { onOpenSession(record) }
                        }
                    }
                    .fixedSize()
                    .disabled(crumb.siblings.isEmpty)
                    .accessibilityIdentifier("task-sessions-\(task.task.id)")
                }
                Button("Monitor") { onMonitor(task.task.id) }
                    .accessibilityIdentifier("task-show-monitor-\(task.task.id)")
                if crumb.session == nil {
                    Button("Inspect") { model.select(task.id.work) }
                }
            }
        }
        .font(.system(size: 12))
        .padding(8)
    }

    private var separator: some View {
        Text("/").foregroundStyle(palette.textSecondary)
    }

    @ViewBuilder
    private func sessionCrumb(_ session: SessionRecord) -> some View {
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
            if crumb.siblings.count > 1 {
                Menu(session.title) {
                    ForEach(crumb.siblings) { record in
                        Button(record.title) { onOpenSession(record) }
                            .accessibilityIdentifier("breadcrumb-session-\(record.id)")
                    }
                }
                .fixedSize()
                .accessibilityIdentifier("breadcrumb-session")
            } else {
                Text(session.title)
                    .fontWeight(.semibold)
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
            Text(session.flowMembership.label)
                .foregroundStyle(palette.textSecondary)
                .help(membershipHelp(session.flowMembership))
                .accessibilityIdentifier("session-flow-membership")
        }
    }

    private func membershipHelp(_ membership: SessionFlowMembership) -> String {
        switch membership {
        case .step(_, _, _, _, _, let current):
            current ? "This conversation is the Flow's current step." : "This conversation belonged to an earlier Flow step."
        case .independent:
            "This conversation is not part of the Task's managed Flow."
        case .unknown(let reason):
            reason
        }
    }
}

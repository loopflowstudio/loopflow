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
                Text("Rename Session").font(Typography.textStrong)
                TextField("Session name", text: Binding(
                    get: { model.navigation.renaming?.text ?? draft.text },
                    set: { model.navigation.renaming?.text = $0 }))
                    .onSubmit { model.commitSessionRename() }
                    .accessibilityIdentifier("session-rename-field")
                HStack {
                    Button("Cancel") { model.cancelSessionRename() }.keyboardShortcut(.cancelAction)
                    Button("Save") { model.commitSessionRename() }.keyboardShortcut(.defaultAction)
                }
            }.padding(20).frame(width: 380)
        }
    }

    @ViewBuilder
    private var bindingForm: some View {
        if let draft = model.navigation.binding {
            VStack(alignment: .leading, spacing: 12) {
                Text(draft.assignedTaskId == nil ? "Bind “\(draft.title)” to a Task" : "Make “\(draft.title)” primary")
                    .font(Typography.textStrong)
                if draft.assignedTaskId == nil {
                    Text("Binding is permanent. This Session cannot be moved to another Task or unbound. No workflow starts.")
                }
                if draft.assignedTaskId != nil {
                    Text("Use this conversation when opening its Task. No workflow starts.")
                    Button("Make primary Task conversation") {
                        Task { await model.commitSessionBinding() }
                    }
                    .disabled(draft.submitting)
                    .accessibilityIdentifier("session-bind-assigned")
                } else if let preview = draft.preview {
                    Text("\(preview.identifier) · \(preview.title)")
                    HStack {
                        Button("Choose another Task") { model.navigation.binding?.preview = nil }
                        Button("Bind to Task") { Task { await model.commitSessionBinding() } }
                            .accessibilityIdentifier("session-bind-confirm")
                    }
                    .disabled(draft.submitting)
                    Text("This becomes the conversation opened by the Task. Other conversations remain associated.")
                } else {
                    TextField("New Task title", text: Binding(
                        get: { model.navigation.binding?.taskTitle ?? "" },
                        set: { model.navigation.binding?.taskTitle = $0 }))
                        .disabled(draft.submitting)
                        .accessibilityIdentifier("session-create-title")
                    Button("Create Task from this conversation") {
                        Task { await model.createTaskFromSession() }
                    }
                    .disabled(draft.submitting || draft.taskTitle.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .accessibilityIdentifier("session-create-task")
                    Text("Keep this chat, worktree, branch and unfinished edits.")
                    Divider()
                    TextField("Task identifier or stable ID", text: Binding(
                        get: { model.navigation.binding?.selector ?? "" },
                        set: { model.navigation.binding?.selector = $0 }))
                        .disabled(draft.submitting)
                        .accessibilityIdentifier("session-bind-target")
                    Button("Review binding") { Task { await model.previewSessionBinding() } }
                        .disabled(draft.submitting || draft.selector.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        .accessibilityIdentifier("session-bind-preview")
                }
                Text("Creation keeps this conversation in place. Binding sets its directory to the Task worktree immediately. Already-running clients may keep their original directory. Files stay where they are.")
                if let error = draft.error {
                    FailureLine("Not bound.", machine: error)
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

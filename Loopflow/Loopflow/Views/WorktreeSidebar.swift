// Sidebar view showing worktrees with status and actions.

import SwiftUI

struct WorktreeSidebar: View {
    @Bindable var appState: AppState
    @State private var showingNewWorktreeSheet = false
    @State private var showingDeleteConfirmation = false
    @State private var worktreeToDelete: Worktree?
    @State private var actionError: String?
    @State private var showingActionError = false

    private let terminalLauncher = TerminalLauncher()

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header

            if appState.worktrees.isEmpty {
                emptyState
            } else {
                worktreeList
            }
        }
        .sheet(isPresented: $showingNewWorktreeSheet) {
            NewWorktreeSheet(appState: appState)
        }
        .confirmationDialog(
            "Delete Worktree",
            isPresented: $showingDeleteConfirmation,
            presenting: worktreeToDelete
        ) { worktree in
            Button("Delete", role: .destructive) {
                Task {
                    do {
                        try await appState.deleteWorktree(worktree)
                    } catch {
                        actionError = "Failed to delete worktree: \(error.localizedDescription)"
                        showingActionError = true
                    }
                }
            }
        } message: { worktree in
            Text("Are you sure you want to delete '\(worktree.branch)'? This will remove the worktree and its local branch.")
        }
        .alert("Error", isPresented: $showingActionError) {
            Button("OK") {
                actionError = nil
            }
        } message: {
            Text(actionError ?? "An error occurred")
        }
    }

    private var header: some View {
        HStack {
            Text("WORKTREES")
                .font(.caption)
                .fontWeight(.semibold)
                .foregroundStyle(.secondary)

            Spacer()

            Button {
                showingNewWorktreeSheet = true
            } label: {
                Image(systemName: "plus")
                    .font(.caption)
            }
            .buttonStyle(.plain)
            .help("New Worktree")
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    private var emptyState: some View {
        VStack(spacing: 8) {
            Text("No worktrees")
                .foregroundStyle(.secondary)
            Text("Click + to create one")
                .font(.caption)
                .foregroundStyle(.tertiary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var worktreeList: some View {
        ScrollView {
            LazyVStack(spacing: 4) {
                ForEach(appState.worktrees) { worktree in
                    WorktreeRow(
                        worktree: worktree,
                        isSelected: appState.selectedWorktree?.id == worktree.id,
                        ideName: ideDisplayName,
                        onSelect: {
                            appState.selectedWorktree = worktree
                        },
                        onDoubleClick: {
                            openInTerminal(worktree)
                        },
                        onOpenTerminal: {
                            openInTerminal(worktree)
                        },
                        onOpenIDE: {
                            openInIDE(worktree)
                        },
                        onOpenFinder: {
                            terminalLauncher.openInFinder(at: URL(fileURLWithPath: worktree.path))
                        },
                        onViewPR: {
                            if let url = worktree.prURL {
                                terminalLauncher.openURL(url)
                            }
                        },
                        onDelete: {
                            worktreeToDelete = worktree
                            showingDeleteConfirmation = true
                        }
                    )
                }
            }
            .padding(.horizontal, 8)
        }
    }

    private func openInTerminal(_ worktree: Worktree) {
        let terminal = appState.config?.terminalApp ?? .warp
        do {
            try terminalLauncher.launchTerminal(terminal, at: URL(fileURLWithPath: worktree.path))
        } catch {
            actionError = "Failed to open terminal: \(error.localizedDescription)"
            showingActionError = true
        }
    }

    private func openInIDE(_ worktree: Worktree) {
        let ide = appState.config?.ideApp ?? .cursor
        let workspace = appState.config?.workspace
        do {
            try terminalLauncher.openInIDE(ide, at: URL(fileURLWithPath: worktree.path), workspace: workspace)
        } catch {
            actionError = "Failed to open \(ide.displayName): \(error.localizedDescription)"
            showingActionError = true
        }
    }

    private var ideDisplayName: String {
        appState.config?.ideApp.displayName ?? "Cursor"
    }
}

struct WorktreeRow: View {
    let worktree: Worktree
    let isSelected: Bool
    let ideName: String
    let onSelect: () -> Void
    let onDoubleClick: () -> Void
    let onOpenTerminal: () -> Void
    let onOpenIDE: () -> Void
    let onOpenFinder: () -> Void
    let onViewPR: () -> Void
    let onDelete: () -> Void

    @State private var isHovering = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(worktree.branch)
                    .fontWeight(.medium)

                Spacer()

                statusBadge
            }

            HStack(spacing: 4) {
                Image(systemName: "arrow.turn.down.right")
                    .font(.caption2)
                    .foregroundStyle(.tertiary)

                Text(worktree.commitsText)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(isSelected ? Color.accentColor.opacity(0.15) : (isHovering ? Color.primary.opacity(0.05) : Color.clear))
        )
        .contentShape(Rectangle())
        .onHover { hovering in
            isHovering = hovering
        }
        .onTapGesture(count: 2) {
            onDoubleClick()
        }
        .onTapGesture {
            onSelect()
        }
        .contextMenu {
            Button("Open in Terminal") {
                onOpenTerminal()
            }
            Button("Open in \(ideName)") {
                onOpenIDE()
            }
            Button("Reveal in Finder") {
                onOpenFinder()
            }

            if worktree.prURL != nil {
                Divider()
                Button("View PR") {
                    onViewPR()
                }
            }

            Divider()

            Button("Delete", role: .destructive) {
                onDelete()
            }
        }
    }

    private var statusBadge: some View {
        HStack(spacing: 4) {
            if worktree.isDirty {
                Circle()
                    .fill(.orange)
                    .frame(width: 6, height: 6)
                Text(worktree.statusText)
                    .font(.caption)
                    .foregroundStyle(.orange)
            } else {
                Image(systemName: "checkmark")
                    .font(.caption2)
                    .foregroundStyle(.green)
                Text("clean")
                    .font(.caption)
                    .foregroundStyle(.green)
            }
        }
    }
}

struct NewWorktreeSheet: View {
    @Bindable var appState: AppState
    @Environment(\.dismiss) private var dismiss

    @State private var branchName = ""
    @State private var baseBranch = "main"
    @State private var isCreating = false
    @State private var errorMessage: String?

    var body: some View {
        VStack(spacing: 20) {
            Text("New Worktree")
                .font(.headline)

            VStack(alignment: .leading, spacing: 8) {
                Text("Branch name")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                TextField("feature-name", text: $branchName)
                    .textFieldStyle(.roundedBorder)
            }

            VStack(alignment: .leading, spacing: 8) {
                Text("Base branch")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Picker("", selection: $baseBranch) {
                    Text("main").tag("main")
                    ForEach(appState.worktrees.filter { $0.branch != "main" }) { wt in
                        Text(wt.branch).tag(wt.branch)
                    }
                }
                .pickerStyle(.menu)
                .labelsHidden()
            }

            if let error = errorMessage {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
            }

            HStack {
                Button("Cancel") {
                    dismiss()
                }
                .keyboardShortcut(.escape)

                Spacer()

                Button("Create") {
                    createWorktree()
                }
                .keyboardShortcut(.defaultAction)
                .disabled(branchName.isEmpty || isCreating)
            }
        }
        .padding(24)
        .frame(width: 320)
    }

    private func createWorktree() {
        isCreating = true
        errorMessage = nil

        Task {
            do {
                try await appState.createWorktree(name: branchName, baseBranch: baseBranch)
                dismiss()
            } catch {
                errorMessage = error.localizedDescription
            }
            isCreating = false
        }
    }
}

#Preview {
    let state = AppState()
    return WorktreeSidebar(appState: state)
        .frame(width: 280, height: 400)
}

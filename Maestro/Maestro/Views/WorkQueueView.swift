// Work queue view for reviewing and managing work items.

import SwiftUI

struct WorkQueueView: View {
    let repoPath: String
    @State private var items: [WorkItem] = []
    @State private var isLoading = false
    @State private var selectedTab: WorkStatus? = nil
    @State private var error: String?

    var body: some View {
        VStack(spacing: 0) {
            // Tab bar
            HStack(spacing: 16) {
                TabButton(title: "All", isSelected: selectedTab == nil) {
                    selectedTab = nil
                }
                TabButton(title: "Proposed", isSelected: selectedTab == .proposed) {
                    selectedTab = .proposed
                }
                TabButton(title: "Approved", isSelected: selectedTab == .approved) {
                    selectedTab = .approved
                }
                TabButton(title: "Active", isSelected: selectedTab == .active) {
                    selectedTab = .active
                }
                Spacer()
                Button(action: loadItems) {
                    Image(systemName: "arrow.clockwise")
                }
                .buttonStyle(.plain)
                .disabled(isLoading)
            }
            .padding(.horizontal)
            .padding(.vertical, 8)

            Divider()

            // Content
            if isLoading && items.isEmpty {
                ProgressView()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let error = error {
                Text(error)
                    .foregroundColor(.secondary)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if filteredItems.isEmpty {
                Text("No work items")
                    .foregroundColor(.secondary)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                List(filteredItems) { item in
                    WorkItemRow(item: item) {
                        await performAction($0, on: item)
                    }
                }
            }
        }
        .task {
            await loadItems()
        }
    }

    private var filteredItems: [WorkItem] {
        if let tab = selectedTab {
            return items.filter { $0.status == tab }
        }
        return items
    }

    private func loadItems() {
        Task {
            await loadItems()
        }
    }

    @MainActor
    private func loadItems() async {
        isLoading = true
        error = nil

        let service = WorkService(repoPath: repoPath)
        do {
            items = try await service.listItems()
        } catch {
            self.error = "Failed to load: \(error.localizedDescription)"
        }

        isLoading = false
    }

    @MainActor
    private func performAction(_ action: ItemAction, on item: WorkItem) async {
        let service = WorkService(repoPath: repoPath)
        do {
            switch action {
            case .approve:
                try await service.approve(item.id)
            case .reject:
                try await service.reject(item.id)
            case .claim:
                try await service.claim(item.id)
            case .release:
                try await service.release(item.id)
            }
            await loadItems()
        } catch {
            self.error = "Action failed: \(error.localizedDescription)"
        }
    }
}

enum ItemAction {
    case approve
    case reject
    case claim
    case release
}

struct WorkItemRow: View {
    let item: WorkItem
    let onAction: (ItemAction) async -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                statusIcon
                Text(item.title)
                    .fontWeight(.medium)
                Spacer()
            }

            if item.isBlocked {
                HStack(spacing: 4) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundColor(.orange)
                    Text(item.blockedOn ?? "Blocked")
                        .font(.caption)
                        .foregroundColor(.orange)
                }
            }

            if item.isHumanClaimed {
                Text("Claimed by human")
                    .font(.caption)
                    .foregroundColor(.blue)
            }

            if !item.description.isEmpty {
                Text(item.description)
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .lineLimit(2)
            }

            // Action buttons
            HStack(spacing: 8) {
                switch item.status {
                case .proposed:
                    Button("Approve") {
                        Task { await onAction(.approve) }
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(.green)
                    .controlSize(.small)

                    Button("Reject") {
                        Task { await onAction(.reject) }
                    }
                    .buttonStyle(.bordered)
                    .tint(.red)
                    .controlSize(.small)

                case .approved, .active:
                    if item.isHumanClaimed {
                        Button("Release") {
                            Task { await onAction(.release) }
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                    } else {
                        Button("Claim") {
                            Task { await onAction(.claim) }
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                    }

                case .done:
                    EmptyView()
                }
            }
            .padding(.top, 4)
        }
        .padding(.vertical, 4)
    }

    @ViewBuilder
    private var statusIcon: some View {
        switch item.status {
        case .proposed:
            Image(systemName: "questionmark.circle")
                .foregroundColor(.gray)
        case .approved:
            Image(systemName: "checkmark.circle")
                .foregroundColor(.green)
        case .active:
            Image(systemName: "play.circle.fill")
                .foregroundColor(.blue)
        case .done:
            Image(systemName: "checkmark.circle.fill")
                .foregroundColor(.green)
        }
    }

}

struct TabButton: View {
    let title: String
    let isSelected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(title)
                .fontWeight(isSelected ? .semibold : .regular)
                .foregroundColor(isSelected ? .accentColor : .secondary)
        }
        .buttonStyle(.plain)
    }
}

#Preview {
    WorkQueueView(repoPath: "/tmp/test-repo")
        .frame(width: 400, height: 500)
}

import Loopflow
import Observation
import SwiftUI

@MainActor
@Observable
final class MobileSessionList {
    private let client: any MobileSessionListingClient
    let task: WorkViewPlan.Content
    private var generation = 0
    private var failedAfter: String?
    private var reading: Task<SessionPage, Error>?
    private(set) var entries: [SessionRecord] = []
    private(set) var next: String?
    private(set) var error: String?
    private(set) var isLoading = false

    init(client: any MobileSessionListingClient, task: WorkViewPlan.Content) {
        self.client = client
        self.task = task
    }

    func refresh() async { await load(after: nil) }
    func retry() async { await load(after: failedAfter) }

    func loadMore() async {
        guard let next, !isLoading else { return }
        await load(after: next)
    }

    func cancel() {
        generation += 1
        reading?.cancel()
        reading = nil
        isLoading = false
    }

    private func load(after: String?) async {
        generation += 1
        let current = generation
        reading?.cancel()
        let request = Task { try await client.sessions(for: task, after: after) }
        reading = request
        isLoading = true
        defer {
            if generation == current { isLoading = false; reading = nil }
        }
        do {
            let page = try await withTaskCancellationHandler {
                try await request.value
            } onCancel: { request.cancel() }
            guard current == generation, !Task.isCancelled else { return }
            if after == nil { entries = page.entries }
            else {
                let incoming = Set(page.entries.map(\.id))
                entries.removeAll { incoming.contains($0.id) }
                entries.append(contentsOf: page.entries)
            }
            next = page.next
            error = nil
            failedAfter = nil
        } catch {
            guard current == generation, !Task.isCancelled else { return }
            self.error = error.localizedDescription
            failedAfter = after
        }
    }
}

struct MobileSessionsView: View {
    @State private var model: MobileSessionList
    let currentID: String?
    let theme: ThemeDefinition
    let choose: (SessionRecord) -> Void
    @Environment(\.dismiss) private var dismiss

    init(client: any MobileSessionListingClient, task: WorkViewPlan.Content, currentID: String?,
         theme: ThemeDefinition, choose: @escaping (SessionRecord) -> Void) {
        _model = State(initialValue: MobileSessionList(client: client, task: task))
        self.currentID = currentID
        self.theme = theme
        self.choose = choose
    }

    var body: some View {
        NavigationStack {
            List {
                Section {
                    if let error = model.error {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Couldn’t update Sessions").font(Typography.strong(15))
                            Text(error).font(Typography.caption(13))
                            if !model.entries.isEmpty {
                                Text("Showing the last received Sessions.").font(Typography.caption(13))
                            }
                            Button("Try again") { Task { await model.retry() } }
                                .disabled(model.isLoading)
                        }
                        .listRowBackground(theme.palette.surface)
                    }
                    ForEach(model.entries) { session in
                        Button {
                            choose(session)
                            dismiss()
                        } label: {
                            HStack(alignment: .top, spacing: 12) {
                                VStack(alignment: .leading, spacing: 6) {
                                    Text(session.title)
                                        .font(Typography.strong(17))
                                        .foregroundStyle(theme.palette.text)
                                    Text([session.provider, session.statusLabel,
                                          session.taskPrimary ? "Primary" : nil].compactMap { $0 }.joined(separator: " · "))
                                        .font(Typography.caption(13))
                                        .foregroundStyle(theme.palette.textSecondary)
                                }
                                .fixedSize(horizontal: false, vertical: true)
                                Spacer(minLength: 0)
                                if session.id == currentID {
                                    Image(systemName: "checkmark")
                                        .accessibilityLabel("Current Session")
                                }
                            }
                            .padding(.vertical, 6)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .contentShape(Rectangle())
                        }
                        .accessibilityElement(children: .combine)
                        .accessibilityHint("View this Session without taking keyboard control")
                        .listRowBackground(theme.palette.background)
                    }
                    if model.next != nil {
                        Button("Load more Sessions") { Task { await model.loadMore() } }
                            .disabled(model.isLoading)
                            .listRowBackground(theme.palette.background)
                    }
                    if model.isLoading {
                        ProgressView("Loading Sessions…")
                            .listRowBackground(theme.palette.background)
                    }
                } header: {
                    Text(model.task.title).font(Typography.caption(13)).textCase(nil)
                }
            }
            .listStyle(.plain)
            .scrollContentBackground(.hidden)
            .background(theme.palette.background)
            .foregroundStyle(theme.palette.text)
            .overlay {
                if model.entries.isEmpty && !model.isLoading && model.error == nil {
                    ContentUnavailableView("No open Sessions", systemImage: "bubble.left.and.bubble.right",
                        description: Text("Open conversations for this Task will appear here."))
                }
            }
            .refreshable { await model.refresh() }
            .navigationTitle("Sessions")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
            }
        }
        .tint(theme.palette.accentInk)
        .task { await model.refresh() }
        .onDisappear { model.cancel() }
    }
}

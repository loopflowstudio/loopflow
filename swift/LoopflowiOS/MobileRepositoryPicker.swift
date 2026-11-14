import Loopflow
import SwiftUI

struct MobileRepositoryPicker: View {
    let model: MobileTaskCreation
    let theme: ThemeDefinition
    @State private var search = ""
    @State private var refresh: Task<Void, Never>?
    @Environment(\.dismiss) private var dismiss

    private var repositories: [PortfolioRepo] {
        let query = search.trimmingCharacters(in: .whitespacesAndNewlines)
        return (model.context?.repositories ?? []).filter {
            query.isEmpty || $0.path.localizedStandardContains(query)
                || $0.displayName.localizedStandardContains(query)
        }.sorted { $0.path.localizedStandardCompare($1.path) == .orderedAscending }
    }

    var body: some View {
        List {
            if let error = model.error {
                Section {
                    Text(error).font(Typography.body(14)).foregroundStyle(theme.palette.warning)
                        .listRowBackground(theme.palette.surface)
                }
            }
            Section {
                ForEach(repositories) { repository in
                    Button {
                        model.selectRepository(repository.path)
                        if model.saveDraft() { dismiss() }
                    } label: {
                        HStack(alignment: .top, spacing: 12) {
                            VStack(alignment: .leading, spacing: 6) {
                                Text(repository.displayName).font(Typography.strong(17))
                                    .foregroundStyle(theme.palette.text)
                                Text(repository.path).font(Typography.caption(13))
                                    .foregroundStyle(theme.palette.textSecondary)
                                if let error = repository.error {
                                    Label(error, systemImage: "exclamationmark.circle")
                                        .font(Typography.body(14)).foregroundStyle(theme.palette.warning)
                                }
                            }
                            .fixedSize(horizontal: false, vertical: true)
                            Spacer(minLength: 0)
                            if repository.path == model.draft.directory {
                                Image(systemName: "checkmark").foregroundStyle(theme.palette.accentInk)
                                    .accessibilityHidden(true)
                            }
                        }
                        .frame(maxWidth: .infinity, minHeight: 44, alignment: .leading)
                        .contentShape(Rectangle())
                        .padding(.vertical, 4)
                    }
                    .disabled(repository.error != nil || model.busy)
                    .accessibilityAddTraits(repository.path == model.draft.directory ? .isSelected : [])
                    .listRowBackground(theme.palette.background)
                }
            } header: {
                Text(model.context?.name ?? "Paired Machine")
                    .font(Typography.caption(12)).foregroundStyle(theme.palette.textSecondary).textCase(nil)
            }
        }
        .listStyle(.plain)
        .scrollContentBackground(.hidden)
        .background(theme.palette.background)
        .overlay {
            if repositories.isEmpty && model.error == nil {
                if model.context?.repositories.isEmpty == true {
                    ContentUnavailableView("No repositories", systemImage: "folder",
                        description: Text("Add a repository in Loopflow on this Machine, then refresh this list."))
                } else {
                    ContentUnavailableView.search(text: search)
                }
            }
        }
        .searchable(text: $search, prompt: "Repository name or path")
        .scrollDismissesKeyboard(.interactively)
        .refreshable { await model.prepare() }
        .navigationTitle("Repository")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                if model.busy {
                    ProgressView().accessibilityLabel("Refreshing repositories")
                } else {
                    Button("Refresh repositories", systemImage: "arrow.clockwise") {
                        refresh = Task { await model.prepare() }
                    }
                }
            }
        }
        .onDisappear { refresh?.cancel() }
    }
}

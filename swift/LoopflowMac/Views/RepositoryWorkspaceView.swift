import Foundation
import Loopflow
import SwiftUI

extension RepositoryWorkspace {
    static func resolve(path: String, query: RegistryQuery) async throws -> Self {
        let localPath = await Task.detached {
            RepoScanner().mainRepository(URL(fileURLWithPath: path))?.normalizedFilePath
        }.value
        guard let localPath else { throw RegistryQueryError("\(path) is not an available local Git repository.") }
        return try await Self(id: query.repositoryIdentity(cwd: localPath), path: localPath)
    }
}

struct TaskWindowChoice: Identifiable {
    let path: String
    let url: URL
    var id: String { path }
}

/// The unbound launch scene is a loading/error surface, never a second workspace.
struct WorkspaceLaunchView: View {
    let open: () async throws -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var error: String?

    var body: some View {
        VStack {
            if let error {
                Text(error).textSelection(.enabled)
                Button("Retry") { Task { await launch() } }
            } else {
                ProgressView("Opening repository…")
            }
        }
        .frame(minWidth: 480, minHeight: 200)
        .task { await launch() }
    }

    private func launch() async {
        error = nil
        do {
            try await open()
            dismiss()
        } catch { self.error = error.localizedDescription }
    }
}

/// Restored scene locators are rechecked before starting readers or providers.
/// A path reused for another plan must not restore that plan's panes here.
struct RepositoryWorkspaceView: View {
    let workspace: RepositoryWorkspace
    let portfolioService: PortfolioService
    let query: RegistryQuery
    let router: WorkLinkRouter
    let openRepository: (String, URL?) -> Void
    @State private var ready = false
    @State private var error: String?

    var body: some View {
        Group {
            if ready {
                RepoView(portfolioService: portfolioService, initialRepoPath: workspace.path,
                         query: query, taskLinks: router, repository: workspace.id,
                         openRepository: openRepository)
            } else if let error {
                VStack {
                    Text(error).textSelection(.enabled)
                    Button("Retry") { Task { await resolve() } }
                }.padding()
            } else {
                ProgressView("Opening repository…")
            }
        }
        .task(id: workspace.id) { await resolve() }
    }

    private func resolve() async {
        error = nil
        do {
            let current = try await RepositoryWorkspace.resolve(path: workspace.path, query: query)
            guard current.id == workspace.id else {
                throw RegistryQueryError("This location now selects another repository plan. Open it explicitly from Open Repo; the restored workspace was not changed.")
            }
            ready = true
        } catch { self.error = error.localizedDescription }
    }
}

/// One utility window owns opening feedback, not a sheet duplicated on every repo.
struct WorkspaceOpeningFeedback: View {
    @Binding var error: String?
    @Binding var choices: [[TaskWindowChoice]]
    let openRepository: (String, URL?) -> Void
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading) {
            if let error {
                Text(error).textSelection(.enabled)
                Button("OK") { self.error = nil; closeIfFinished() }
            } else if let candidates = choices.first {
                Text("Choose the Task's repository").font(.headline)
                ForEach(candidates) { choice in
                    Button(choice.path) {
                        choices.removeFirst()
                        openRepository(choice.path, choice.url)
                        closeIfFinished()
                    }
                }
                Button("Cancel") { choices.removeFirst(); closeIfFinished() }
            } else {
                Text("No pending workspace requests")
                Button("Close") { dismiss() }
            }
        }.padding()
    }

    private func closeIfFinished() {
        if error == nil, choices.isEmpty { dismiss() }
    }
}

import Foundation
import Loopflow
import SwiftUI

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
    @Environment(\.dismiss) private var dismiss
    @State private var reading: WorkReading<RepositoryWorkspace> = .loading
    @State private var openingRequests: Set<UUID> = []

    var body: some View {
        let requests = router.workspaceRequests(workspace.id)
        Group {
            switch reading {
            case .available(let resolved):
                RepoView(portfolioService: portfolioService, initialRepoPath: resolved.path,
                         query: query, taskLinks: router, repository: resolved.id, openingRequests: openingRequests,
                         openRepository: openRepository)
            case .unavailable(_, let reason):
                VStack {
                    Text(reason).textSelection(.enabled)
                    Button("Retry") { Task { await resolve() } }
                }.padding()
            case .loading:
                ProgressView("Opening repository…")
            }
        }
        .task(id: requests) {
            // A new request may arrive between validation and registration.
            // Reuse that now-registered shell without unmounting retained content.
            if case .available = reading, router.hasWindow(workspace.id) {
                router.confirmRegisteredWorkspace(workspace.id, requests: requests)
                return
            }
            await resolve()
        }
    }

    private func resolve() async {
        reading = .loading
        let requests = router.workspaceRequests(workspace.id)
        do {
            let current = try await router.resolveWorkspace(workspace, query: query)
            guard !Task.isCancelled else { return }
            guard current.id == workspace.id else {
                // This restored shell has not mounted any panes. Its retained
                // locator belongs to the scene already reserved by the router.
                openRepository(current.path, nil)
                dismiss()
                return
            }
            openingRequests = requests
            reading = .available(current)
        } catch {
            guard !Task.isCancelled, !(error is CancellationError) else { return }
            reading = .unavailable(lastGood: nil, reason: error.localizedDescription)
        }
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

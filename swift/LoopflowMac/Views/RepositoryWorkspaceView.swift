import Foundation
import Loopflow
import SwiftUI

/// The unbound launch scene is a loading/error surface, never a second workspace.
struct WorkspaceLaunchView: View {
    var activation = AppActivation.shared
    let open: () async throws -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var error: String?

    var body: some View {
        VStack {
            if case .failed(let failure) = activation.state {
                Text(failure).textSelection(.enabled)
                Button("Retry") { Task { await launch() } }
            } else if activation.state == .activating {
                ProgressView("Setting up Loopflow…")
            } else if let error {
                Text(error).textSelection(.enabled)
                Button("Retry") { Task { await launch() } }
            } else {
                ProgressView("Opening repository…")
            }
        }
        .frame(minWidth: 480, minHeight: 200)
        .task { await launch() }
    }

    func launch() async {
        error = nil
        do {
            try await activation.activated()
            try await open()
            dismiss()
        } catch { self.error = error.localizedDescription }
    }
}

/// Retained content mounts independently of resolution; unknown scene locators
/// still need an identity reading before they can supply a workspace.
struct RepositoryWorkspaceView: View {
    let workspace: RepositoryWorkspace
    let portfolioService: PortfolioService
    let workspaces: RepositoryWorkspaces
    let query: RegistryQuery
    let router: WorkLinkRouter
    let openRepository: (String, LinkedWork?) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var resolutionError: String?
    @State private var validatedRequests: Set<UUID>?

    var body: some View {
        let requests = router.workspaceRequests(workspace.id)
        RepositoryWorkspaceContent(workspace: router.retainedWorkspace(workspace.id), error: resolutionError,
                                   retry: { Task { await resolve() } }) { resolved in
            RepoView(portfolioService: portfolioService,
                     sessionWorkspaces: workspaces.registry(for: resolved.id), initialRepoPath: resolved.path,
                     query: query, taskLinks: router, repository: resolved.id, openingRequests: validatedRequests ?? [],
                     openRepository: openRepository)
        }
        .task(id: requests) {
            // A new request may arrive between validation and registration.
            // Reuse that now-registered shell without unmounting retained content.
            if validatedRequests != nil, router.hasWindow(workspace.id) {
                router.confirmRegisteredWorkspace(workspace.id, requests: requests)
                return
            }
            await resolve()
        }
    }

    private func resolve() async {
        resolutionError = nil
        validatedRequests = nil
        let requests = router.workspaceRequests(workspace.id)
        do {
            let current = try await router.resolveWorkspace(workspace, query: query)
            guard !Task.isCancelled else { return }
            guard current.id == workspace.id else {
                // The locator belongs to a scene already reserved by the router.
                openRepository(current.path, nil)
                dismiss()
                return
            }
            validatedRequests = requests
        } catch {
            guard !Task.isCancelled, !(error is CancellationError) else { return }
            resolutionError = error.localizedDescription
        }
    }
}

/// Keep the content at one structural position through refresh and failure.
struct RepositoryWorkspaceContent<Content: View>: View {
    let workspace: RepositoryWorkspace?
    let error: String?
    let retry: () -> Void
    @ViewBuilder let content: (RepositoryWorkspace) -> Content

    var body: some View {
        VStack(spacing: 0) {
            if let workspace {
                content(workspace)
            } else if error == nil {
                ProgressView("Opening repository…")
            }
            if let error {
                HStack {
                    Text(error).textSelection(.enabled)
                    Button("Retry", action: retry)
                }.padding()
            }
        }
    }
}

/// One utility window owns opening feedback, not a sheet duplicated on every repo.
struct WorkspaceOpeningFeedback: View {
    @Binding var error: String?
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading) {
            if let error {
                Text(error).textSelection(.enabled)
                Button("OK") { self.error = nil; dismiss() }
            } else {
                Text("No pending workspace requests")
                Button("Close") { dismiss() }
            }
        }.padding()
    }
}

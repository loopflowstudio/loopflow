// Main content view with sidebar and prompt launcher.

import SwiftUI

struct ContentView: View {
    @Bindable var appState: AppState
    @State private var showingError = false

    var body: some View {
        NavigationSplitView {
            WorktreeSidebar(appState: appState)
                .navigationSplitViewColumnWidth(min: 240, ideal: 280, max: 360)
        } detail: {
            if appState.isLoading {
                ProgressView("Loading...")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if appState.currentRepo != nil {
                PromptLauncher(appState: appState)
            } else {
                // No repo loaded yet - show loading placeholder
                ProgressView("Opening repository...")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .onChange(of: appState.errorMessage) { _, newValue in
            showingError = newValue != nil
        }
        .alert("Error", isPresented: $showingError) {
            Button("OK") {
                appState.errorMessage = nil
            }
        } message: {
            Text(appState.errorMessage ?? "An unknown error occurred")
        }
        .navigationTitle(appState.currentRepo?.lastPathComponent ?? "Loopflow Maestro")
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    Task { await appState.refreshWorktrees() }
                } label: {
                    Image(systemName: "arrow.clockwise")
                }
                .help("Refresh")
            }

            ToolbarItem(placement: .primaryAction) {
                if let repo = appState.currentRepo {
                    Text(repo.lastPathComponent)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        }
    }
}

#Preview {
    ContentView(appState: AppState())
}

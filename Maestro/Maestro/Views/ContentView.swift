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
                WelcomeView(appState: appState)
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
                Menu {
                    Button("Open Folder...") {
                        openFolder()
                    }
                    .keyboardShortcut("o", modifiers: .command)

                    Divider()

                    if let repo = appState.currentRepo {
                        Text(repo.path())
                            .font(.caption)
                    }
                } label: {
                    Image(systemName: "gearshape")
                }
            }
        }
        .onAppear {
            // Try to open the current directory on launch
            if appState.currentRepo == nil {
                openCurrentDirectory()
            }
        }
    }

    private func openFolder() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.message = "Choose a repository folder"

        if panel.runModal() == .OK, let url = panel.url {
            Task {
                await appState.openRepo(url)
            }
        }
    }

    private func openCurrentDirectory() {
        // Use current working directory or home
        let cwd = FileManager.default.currentDirectoryPath
        let url = URL(fileURLWithPath: cwd)

        // Check if it's a git repo
        if FileManager.default.fileExists(atPath: url.appendingPathComponent(".git").path()) {
            Task {
                await appState.openRepo(url)
            }
        }
    }
}

struct WelcomeView: View {
    @Bindable var appState: AppState

    var body: some View {
        VStack(spacing: 24) {
            Image(systemName: "folder.badge.gearshape")
                .font(.system(size: 64))
                .foregroundStyle(.secondary)

            VStack(spacing: 8) {
                Text("Open a Repository")
                    .font(.title2)
                    .fontWeight(.semibold)

                Text("Select a folder with .lf/config.yaml to get started")
                    .foregroundStyle(.secondary)
            }

            Button("Open Folder...") {
                openFolder()
            }
            .buttonStyle(.borderedProminent)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func openFolder() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.message = "Choose a repository folder"

        if panel.runModal() == .OK, let url = panel.url {
            Task {
                await appState.openRepo(url)
            }
        }
    }
}

#Preview {
    ContentView(appState: AppState())
}

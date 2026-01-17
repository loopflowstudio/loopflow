// Main content view with sidebar and prompt launcher.

import SwiftUI

struct ContentView: View {
    @Bindable var appState: AppState
    @State private var showingError = false

    private func pipelineEditorBinding(_ pipeline: PipelineDef) -> some View {
        PipelineEditor(
            pipeline: Binding(
                get: { appState.selectedPipeline ?? pipeline },
                set: { appState.selectedPipeline = $0 }
            ),
            availableTasks: appState.prompts,
            availablePipelines: appState.pipelines.filter { $0.name != pipeline.name },
            onSave: {
                if let selected = appState.selectedPipeline {
                    appState.savePipeline(selected)
                }
            },
            onRun: {
                // TODO: Launch pipeline in terminal
            },
            onDelete: {
                if let selected = appState.selectedPipeline {
                    appState.deletePipeline(selected)
                }
            }
        )
    }

    var body: some View {
        NavigationSplitView {
            WorktreeSidebar(appState: appState)
                .navigationSplitViewColumnWidth(min: 240, ideal: 280, max: 360)
        } detail: {
            if appState.isLoading {
                ProgressView("Loading...")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if appState.currentRepo != nil {
                VStack(spacing: 0) {
                    if Flags.beta, let pipeline = appState.selectedPipeline {
                        pipelineEditorBinding(pipeline)
                    } else {
                        PromptLauncher(appState: appState)
                    }
                    ResultsPanel(appState: appState)
                }
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
                .help("Refresh workspaces and tasks")
            }
        }
    }
}

#Preview {
    ContentView(appState: AppState())
}

// Repo-scoped wave list.

import SwiftUI
import LoopflowCore

struct ContentView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    @State private var showingError = false

    private var currentRepoPath: String? {
        if let currentRepo = repoState.currentRepo {
            return currentRepo.normalizedFilePath
        }
        return repoState.repoTarget?.path.normalizedFilePath
    }

    private var filteredWaves: [WaveViewModel] {
        repoFilteredWaves(repoState.waves, currentRepoPath: currentRepoPath)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            Divider()

            if repoState.isLoading {
                loadingState
            } else if currentRepoPath == nil {
                openingState
            } else if filteredWaves.isEmpty {
                emptyState
            } else {
                waveList
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(palette.background)
        .navigationTitle(repoState.currentRepo?.lastPathComponent ?? "Loopflow")
        .onChange(of: repoState.errorMessage) { _, newValue in
            showingError = newValue != nil
        }
        .alert("Error", isPresented: $showingError) {
            Button("OK") {
                repoState.errorMessage = nil
            }
        } message: {
            Text(repoState.errorMessage ?? "An unknown error occurred")
        }
    }

    private var header: some View {
        HStack(alignment: .firstTextBaseline) {
            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text("Waves")
                    .font(Typography.sectionTitle(24))
                    .foregroundStyle(palette.text)

                Text(repoState.currentRepo?.path(percentEncoded: false) ?? "Opening repository...")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }

            Spacer()

            Text(repoState.lfdConnected ? "Connected" : "Disconnected")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
                .padding(.horizontal, Spacing.sm)
                .padding(.vertical, Spacing.xs)
                .background(
                    RoundedRectangle(cornerRadius: CornerRadius.sm)
                        .fill(palette.text.opacity(0.06))
                )
        }
        .padding(.horizontal, Spacing.xl)
        .padding(.vertical, Spacing.lg)
    }

    private var loadingState: some View {
        VStack(spacing: Spacing.md) {
            ProgressView()
            Text("Loading waves...")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var openingState: some View {
        VStack(spacing: Spacing.md) {
            ProgressView()
            Text("Opening repository...")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var emptyState: some View {
        Text("No waves for this repo")
            .font(Typography.body())
            .foregroundStyle(palette.textSecondary)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var waveList: some View {
        ScrollView {
            LazyVStack(spacing: 0) {
                ForEach(filteredWaves) { wave in
                    RepoWaveRow(wave: wave)
                    Divider()
                }
            }
            .padding(.horizontal, Spacing.xl)
            .padding(.top, Spacing.md)
        }
        .accessibilityIdentifier("repo-wave-list")
    }
}

func repoFilteredWaves(_ waves: [WaveViewModel], currentRepoPath: String?) -> [WaveViewModel] {
    guard let currentRepoPath else { return [] }
    return waves.filter { wave in
        wave.repo.normalizedFilePath == currentRepoPath
    }
}

private struct RepoWaveRow: View {
    let wave: WaveViewModel

    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Spacing.md) {
            Image(systemName: wave.status.icon)
                .foregroundStyle(wave.status.color)
                .frame(width: 16)
                .accessibilityHidden(true)

            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text(wave.displayName)
                    .font(Typography.body())
                    .fontWeight(.medium)
                    .foregroundStyle(palette.text)
                    .lineLimit(1)
                    .accessibilityIdentifier("wave-name")

                Text(wave.statusText)
                    .font(Typography.caption())
                    .foregroundStyle(wave.status.color)
                    .accessibilityIdentifier("wave-status")
            }

            Spacer()
        }
        .padding(.vertical, Spacing.md)
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(wave.displayName), \(wave.statusText)")
    }
}

#Preview {
    ContentView()
        .environment(RepoState())
        .environment(OutputBuffer())
        .environment(KeyboardRouter())
}

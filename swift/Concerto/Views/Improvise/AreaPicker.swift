// AreaPicker - select an area for the wave (recent, browse, or infer from branch).

import SwiftUI
import LoopflowCore
import AppKit

struct AreaPicker: View {
    let wave: Wave

    @Environment(RepoState.self) private var repoState
    @Environment(\.colorScheme) private var colorScheme

    @State private var inferredPaths: [String] = []

    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }
    private let recentAreasService = RecentAreasService()

    private var recentAreas: [String] {
        guard let repo = repoState.currentRepo else { return [] }
        return recentAreasService.recentAreas(for: repo)
    }

    var body: some View {
        VStack(spacing: Spacing.xxl) {
            // Header
            VStack(spacing: Spacing.sm) {
                Image(systemName: "folder.badge.questionmark")
                    .font(.system(size: 48))
                    .foregroundStyle(palette.accent)

                Text("Pick an Area")
                    .font(.title2)
                    .fontWeight(.semibold)

                Text("Choose where to focus this wave. The area defines which files and context are included.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 400)
            }
            .padding(.top, Spacing.xxxl)

            VStack(spacing: Spacing.lg) {
                // Recent areas
                if !recentAreas.isEmpty {
                    VStack(alignment: .leading, spacing: Spacing.sm) {
                        Text("Recent")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .padding(.leading, Spacing.xs)

                        VStack(spacing: Spacing.xs) {
                            ForEach(recentAreas, id: \.self) { area in
                                Button {
                                    selectArea(area)
                                } label: {
                                    areaRow(path: area, icon: "clock")
                                }
                                .buttonStyle(.plain)
                            }
                        }
                    }
                }

                // Inferred from branch
                if wave.hasDiff && !inferredPaths.isEmpty {
                    VStack(alignment: .leading, spacing: Spacing.sm) {
                        Text("From Current Changes")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .padding(.leading, Spacing.xs)

                        VStack(spacing: Spacing.xs) {
                            ForEach(inferredPaths, id: \.self) { path in
                                Button {
                                    selectArea(path)
                                } label: {
                                    areaRow(path: path, icon: "arrow.triangle.branch")
                                }
                                .buttonStyle(.plain)
                            }
                        }
                    }
                }

                // Browse button
                VStack(alignment: .leading, spacing: Spacing.sm) {
                    Text("Browse")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .padding(.leading, Spacing.xs)

                    Button {
                        browseForFolder()
                    } label: {
                        HStack(spacing: Spacing.md) {
                            Image(systemName: "folder")
                                .font(.system(size: 16))
                                .foregroundStyle(palette.accent)
                                .frame(width: 24)

                            Text("Choose folder...")
                                .font(.subheadline)

                            Spacer()

                            Image(systemName: "chevron.right")
                                .font(.caption)
                                .foregroundStyle(.tertiary)
                        }
                        .padding(.horizontal, Spacing.md)
                        .padding(.vertical, Spacing.md)
                        .background(palette.surface)
                        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                    }
                    .buttonStyle(.plain)
                }

                // Root directory quick option
                VStack(alignment: .leading, spacing: Spacing.sm) {
                    Text("Quick Options")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .padding(.leading, Spacing.xs)

                    Button {
                        selectArea(".")
                    } label: {
                        areaRow(path: "Entire repository", icon: "house", actualPath: ".")
                    }
                    .buttonStyle(.plain)
                }
            }
            .frame(maxWidth: 400)

            Spacer()
        }
        .padding(Spacing.xl)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(palette.background)
        .task {
            await loadInferredPaths()
        }
    }

    private func areaRow(path: String, icon: String, actualPath: String? = nil) -> some View {
        HStack(spacing: Spacing.md) {
            Image(systemName: icon)
                .font(.system(size: 16))
                .foregroundStyle(palette.accent)
                .frame(width: 24)

            Text(path)
                .font(.subheadline)
                .lineLimit(1)

            Spacer()
        }
        .padding(.horizontal, Spacing.md)
        .padding(.vertical, Spacing.md)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        .contentShape(Rectangle())
    }

    private func selectArea(_ area: String) {
        guard let repo = repoState.currentRepo else { return }

        // Save to recent
        recentAreasService.addRecentArea(area, for: repo)

        // Update wave
        Task {
            try? await repoState.updateWave(wave, area: [area])
        }
    }

    private func browseForFolder() {
        guard let repo = repoState.currentRepo else { return }

        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.directoryURL = repo
        panel.message = "Select a folder to focus on"
        panel.prompt = "Select"

        if panel.runModal() == .OK, let selectedURL = panel.url {
            // Convert to relative path from repo
            let relativePath = selectedURL.path.replacingOccurrences(of: repo.path + "/", with: "")
            let area = relativePath.isEmpty ? "." : relativePath
            selectArea(area)
        }
    }

    private func loadInferredPaths() async {
        guard wave.hasDiff, let worktreePath = wave.worktreePath else { return }

        let worktreeURL = URL(fileURLWithPath: worktreePath)
        let worktreeService = WorktreeService()

        do {
            let stats = try await worktreeService.getDiffStats("main...HEAD", in: worktreeURL)

            // Extract unique directories
            var dirs = Set<String>()
            for stat in stats {
                let dir = stat.directory.isEmpty ? "." : stat.directory
                dirs.insert(dir)
            }

            // Sort and limit
            inferredPaths = Array(dirs.sorted().prefix(3))
        } catch {
            inferredPaths = []
        }
    }
}

#Preview {
    let repoState = RepoState()
    repoState.configureMockWaves()
    repoState.currentRepo = URL(fileURLWithPath: "/tmp/test-repo")

    let wave = Wave(
        id: "test",
        name: "test-wave",
        area: nil,
        flow: "design",
        repo: "/tmp/test-repo",
        hasDiff: true,
        recentSteps: []
    )

    return AreaPicker(wave: wave)
        .environment(repoState)
        .frame(width: 500, height: 600)
}

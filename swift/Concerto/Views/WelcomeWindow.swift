// Welcome window with recent repositories list.

import SwiftUI
import LoopflowCore

struct WelcomeWindow: View {
    let recentsService: RecentsService
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismiss) private var dismiss
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(spacing: 32) {
            // Header
            VStack(spacing: 12) {
                Image(systemName: "wand.and.sparkles")
                    .font(Typography.heroTitle(48))
                    .foregroundStyle(.tint)

                Text("Loopflow Concerto")
                    .font(Typography.heroTitle())
                    .fontWeight(.semibold)
            }

            // Recent repos
            if !recentsService.recentRepos.isEmpty {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Recent Repositories")
                        .font(Typography.sectionTitle())
                        .padding(.horizontal, 4)

                    VStack(spacing: 4) {
                        ForEach(recentsService.recentRepos) { recent in
                            Button {
                                openRepo(recent.url)
                            } label: {
                                HStack {
                                    Image(systemName: "folder")
                                        .foregroundStyle(palette.textSecondary)
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(recent.displayName)
                                            .fontWeight(.medium)
                                        Text(recent.path)
                                            .font(Typography.caption())
                                            .foregroundStyle(palette.textSecondary)
                                            .lineLimit(1)
                                            .truncationMode(.middle)
                                    }
                                    Spacer()
                                    Image(systemName: "chevron.right")
                                        .foregroundStyle(palette.textSecondary)
                                }
                                .padding(.horizontal, 12)
                                .padding(.vertical, 8)
                                .contentShape(Rectangle())
                            }
                            .buttonStyle(.plain)
                            .background(
                                RoundedRectangle(cornerRadius: CornerRadius.sm)
                                    .fill(palette.surface)
                            )
                        }
                    }
                }
                .frame(maxWidth: 360)
            }

            // Open folder button
            Button {
                openFolder()
            } label: {
                Label("Open Folder...", systemImage: "folder.badge.plus")
            }
            .buttonStyle(.borderedProminent)
            .keyboardShortcut("o", modifiers: .command)
        }
        .padding(Spacing.xxxl + Spacing.sm)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(palette.background)
    }

    private func openRepo(_ url: URL) {
        openWindow(id: "repo", value: url)
        dismiss()
    }

    private func openFolder() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.message = "Choose a repository folder"

        if panel.runModal() == .OK, let url = panel.url {
            openRepo(url)
        }
    }
}

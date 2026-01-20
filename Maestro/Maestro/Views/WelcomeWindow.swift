// Welcome window with recent repositories list.

import SwiftUI

struct WelcomeWindow: View {
    let recentsService: RecentsService
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 32) {
            // Header
            VStack(spacing: 12) {
                Image(systemName: "wand.and.sparkles")
                    .font(.system(size: 48))
                    .foregroundStyle(.tint)

                Text("Loopflow Maestro")
                    .font(.title)
                    .fontWeight(.semibold)

                Text("Tell it what to build. It writes the code.")
                    .foregroundStyle(.secondary)
            }

            // Recent repos
            if !recentsService.recentRepos.isEmpty {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Recent Repositories")
                        .font(.headline)
                        .padding(.horizontal, 4)

                    VStack(spacing: 4) {
                        ForEach(recentsService.recentRepos) { recent in
                            Button {
                                openRepo(recent.url)
                            } label: {
                                HStack {
                                    Image(systemName: "folder")
                                        .foregroundStyle(.secondary)
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(recent.displayName)
                                            .fontWeight(.medium)
                                        Text(recent.path)
                                            .font(.caption)
                                            .foregroundStyle(.tertiary)
                                            .lineLimit(1)
                                            .truncationMode(.middle)
                                    }
                                    Spacer()
                                    Image(systemName: "chevron.right")
                                        .foregroundStyle(.tertiary)
                                }
                                .padding(.horizontal, 12)
                                .padding(.vertical, 8)
                                .contentShape(Rectangle())
                            }
                            .buttonStyle(.plain)
                            .background(
                                RoundedRectangle(cornerRadius: 6)
                                    .fill(Color.loopflowCream)
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
        .padding(40)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color.white)
        .preferredColorScheme(.light)
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

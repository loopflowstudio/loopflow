// Sheet for creating a new agent.

import SwiftUI

struct NewAgentSheet: View {
    let onCreate: (String, String, URL) -> Void

    @Environment(\.dismiss) private var dismiss

    @State private var name: String = ""
    @State private var emoji: String = ""
    @State private var repoPath: String = ""
    @State private var showingFilePicker = false

    private let commonEmojis = ["🎯", "🔄", "📊", "🧹", "🚀", "🔧", "📝", "🧪"]

    private var isValid: Bool {
        !name.isEmpty && !repoPath.isEmpty
    }

    var body: some View {
        VStack(spacing: 24) {
            // Header
            Text("New Agent")
                .font(.title2)
                .fontWeight(.semibold)

            // Form
            VStack(alignment: .leading, spacing: 16) {
                // Name
                VStack(alignment: .leading, spacing: 6) {
                    Text("Name")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    TextField("my-agent", text: $name)
                        .textFieldStyle(.roundedBorder)
                }

                // Emoji
                VStack(alignment: .leading, spacing: 6) {
                    Text("Emoji")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    HStack(spacing: 8) {
                        TextField("🎯", text: $emoji)
                            .textFieldStyle(.roundedBorder)
                            .frame(width: 50)

                        ForEach(commonEmojis, id: \.self) { e in
                            Button {
                                emoji = e
                            } label: {
                                Text(e)
                                    .font(.title3)
                            }
                            .buttonStyle(.plain)
                            .opacity(emoji == e ? 1 : 0.5)
                        }
                    }
                }

                // Repo
                VStack(alignment: .leading, spacing: 6) {
                    Text("Repository")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    HStack {
                        TextField("~/src/myapp", text: $repoPath)
                            .textFieldStyle(.roundedBorder)

                        Button("Browse") {
                            showingFilePicker = true
                        }
                    }
                }
            }

            Spacer()

            // Actions
            HStack {
                Button("Cancel") {
                    dismiss()
                }
                .keyboardShortcut(.escape)

                Spacer()

                Button("Create") {
                    let url = URL(fileURLWithPath: (repoPath as NSString).expandingTildeInPath)
                    onCreate(name, emoji.isEmpty ? "🤖" : emoji, url)
                    dismiss()
                }
                .buttonStyle(.borderedProminent)
                .disabled(!isValid)
                .keyboardShortcut(.return)
            }
        }
        .padding(24)
        .frame(width: 400, height: 340)
        .fileImporter(
            isPresented: $showingFilePicker,
            allowedContentTypes: [.folder],
            allowsMultipleSelection: false
        ) { result in
            if case .success(let urls) = result, let url = urls.first {
                repoPath = url.path
            }
        }
    }
}

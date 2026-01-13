// Main content view for composing and launching lf commands.

import SwiftUI

struct PromptLauncher: View {
    @Bindable var appState: AppState

    private let terminalLauncher = TerminalLauncher()

    var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            promptInput

            if !appState.prompts.isEmpty {
                promptCards
            }

            contextSection

            modeSection

            Spacer()

            launchButton
        }
        .padding(24)
    }

    // MARK: - Prompt Input

    private var promptInput: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("What do you want to build?")
                .font(.headline)
                .foregroundStyle(.secondary)

            HStack {
                TextField("Add dark mode toggle to settings", text: $appState.promptArgs)
                    .textFieldStyle(.plain)
                    .font(.title3)

                Text("⌘↵")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(RoundedRectangle(cornerRadius: 4).fill(.quaternary))
            }
            .padding(16)
            .background(
                RoundedRectangle(cornerRadius: 12)
                    .fill(.background)
                    .shadow(color: .black.opacity(0.05), radius: 2, y: 1)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(.quaternary, lineWidth: 1)
            )
        }
    }

    // MARK: - Prompt Cards

    private var promptCards: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("PROMPTS")
                .font(.caption)
                .fontWeight(.semibold)
                .foregroundStyle(.secondary)

            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 12) {
                    ForEach(appState.prompts) { prompt in
                        PromptCardView(
                            prompt: prompt,
                            isSelected: appState.selectedPrompt?.id == prompt.id
                        ) {
                            if appState.selectedPrompt?.id == prompt.id {
                                appState.selectedPrompt = nil
                            } else {
                                appState.selectedPrompt = prompt
                                appState.runMode = prompt.defaultMode
                            }
                        }
                    }
                }
            }
        }
    }

    // MARK: - Context Section

    private var contextSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("CONTEXT")
                    .font(.caption)
                    .fontWeight(.semibold)
                    .foregroundStyle(.secondary)

                Spacer()

                Text("\(appState.estimatedTokens.formatted()) tokens")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            VStack(alignment: .leading, spacing: 8) {
                Toggle("Include diff against main", isOn: $appState.includeDiff)
                    .toggleStyle(.checkbox)

                Toggle("Repository docs (README, STYLE)", isOn: .constant(true))
                    .toggleStyle(.checkbox)
                    .disabled(true)

                if let repo = appState.currentRepo {
                    ContextPicker(
                        repoURL: repo,
                        selectedFolders: $appState.selectedContextFolders
                    )
                }
            }
        }
        .onChange(of: appState.includeDiff) {
            Task { await appState.estimateTokens() }
        }
        .onChange(of: appState.selectedContextFolders) {
            Task { await appState.estimateTokens() }
        }
    }

    // MARK: - Mode Section

    private var modeSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("MODE")
                .font(.caption)
                .fontWeight(.semibold)
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Image(systemName: appState.runMode == .auto ? "largecircle.fill.circle" : "circle")
                        .foregroundStyle(appState.runMode == .auto ? Color.accentColor : Color.secondary)
                    Text("Auto (non-interactive, streams output)")
                }
                .contentShape(Rectangle())
                .onTapGesture {
                    appState.runMode = .auto
                }

                HStack {
                    Image(systemName: appState.runMode == .interactive ? "largecircle.fill.circle" : "circle")
                        .foregroundStyle(appState.runMode == .interactive ? Color.accentColor : Color.secondary)
                    Text("Interactive (full conversation)")
                }
                .contentShape(Rectangle())
                .onTapGesture {
                    appState.runMode = .interactive
                }
            }
        }
    }

    // MARK: - Launch Button

    private var launchButton: some View {
        HStack {
            Spacer()

            Button {
                launchInTerminal()
            } label: {
                HStack {
                    Text("Run in Terminal")
                    Text("⌘↵")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .padding(.horizontal, 20)
                .padding(.vertical, 10)
            }
            .buttonStyle(.borderedProminent)
            .keyboardShortcut(.return, modifiers: .command)
        }
    }

    // MARK: - Actions

    private func launchInTerminal() {
        guard let repo = appState.currentRepo else { return }

        let terminal = appState.config?.terminalApp ?? .warp
        let command = appState.buildCommand()

        // Use selected worktree path if available, otherwise repo root
        let workPath: URL
        if let wt = appState.selectedWorktree {
            workPath = URL(fileURLWithPath: wt.path)
        } else {
            workPath = repo
        }

        try? terminalLauncher.launchTerminal(terminal, at: workPath, command: command)
    }
}

// MARK: - Prompt Card View

struct PromptCardView: View {
    let prompt: PromptCard
    let isSelected: Bool
    let onSelect: () -> Void

    @State private var isHovering = false

    var body: some View {
        VStack(spacing: 8) {
            Text(prompt.displayName)
                .font(.subheadline)
                .fontWeight(.medium)

            Text(prompt.defaultMode == .auto ? "auto" : "interactive")
                .font(.caption2)
                .foregroundStyle(.secondary)
        }
        .frame(width: 90, height: 70)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(isSelected ? Color.accentColor.opacity(0.15) : (isHovering ? Color.primary.opacity(0.05) : Color.clear))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(isSelected ? Color.accentColor : Color.gray.opacity(0.3), lineWidth: 1)
        )
        .contentShape(Rectangle())
        .onHover { hovering in
            isHovering = hovering
        }
        .onTapGesture {
            onSelect()
        }
    }
}

// MARK: - Context Picker

struct ContextPicker: View {
    let repoURL: URL
    @Binding var selectedFolders: Set<URL>

    @State private var expandedFolders: Set<URL> = []
    @State private var rootContents: [URL] = []

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            ForEach(rootContents, id: \.self) { url in
                ContextItemView(
                    url: url,
                    repoURL: repoURL,
                    selectedFolders: $selectedFolders,
                    expandedFolders: $expandedFolders,
                    depth: 0
                )
            }
        }
        .onAppear {
            loadRootContents()
        }
    }

    private func loadRootContents() {
        let fm = FileManager.default
        guard let contents = try? fm.contentsOfDirectory(at: repoURL, includingPropertiesForKeys: [.isDirectoryKey]) else {
            return
        }

        rootContents = contents
            .filter { url in
                let name = url.lastPathComponent
                // Hide hidden files and common ignored directories
                return !name.hasPrefix(".") &&
                       !["node_modules", "__pycache__", "build", "dist", ".git"].contains(name)
            }
            .sorted { $0.lastPathComponent < $1.lastPathComponent }
    }
}

struct ContextItemView: View {
    let url: URL
    let repoURL: URL
    @Binding var selectedFolders: Set<URL>
    @Binding var expandedFolders: Set<URL>
    let depth: Int

    @State private var children: [URL] = []

    private var isDirectory: Bool {
        (try? url.resourceValues(forKeys: [.isDirectoryKey]))?.isDirectory ?? false
    }

    private var isSelected: Bool {
        selectedFolders.contains(url)
    }

    private var isExpanded: Bool {
        expandedFolders.contains(url)
    }

    private var relativePath: String {
        url.path().replacingOccurrences(of: repoURL.path() + "/", with: "")
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 4) {
                if isDirectory {
                    Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                        .frame(width: 12)
                        .contentShape(Rectangle())
                        .onTapGesture {
                            toggleExpanded()
                        }
                } else {
                    Spacer()
                        .frame(width: 12)
                }

                Image(systemName: isSelected ? "checkmark.square.fill" : "square")
                    .foregroundStyle(isSelected ? Color.accentColor : Color.secondary)
                    .contentShape(Rectangle())
                    .onTapGesture {
                        toggleSelection()
                    }

                Text(url.lastPathComponent)
                    .font(.caption)

                Spacer()
            }
            .padding(.leading, CGFloat(depth * 16))

            if isDirectory && isExpanded {
                ForEach(children, id: \.self) { child in
                    ContextItemView(
                        url: child,
                        repoURL: repoURL,
                        selectedFolders: $selectedFolders,
                        expandedFolders: $expandedFolders,
                        depth: depth + 1
                    )
                }
            }
        }
    }

    private func toggleSelection() {
        if isSelected {
            selectedFolders.remove(url)
        } else {
            selectedFolders.insert(url)
        }
    }

    private func toggleExpanded() {
        if isExpanded {
            expandedFolders.remove(url)
        } else {
            expandedFolders.insert(url)
            loadChildren()
        }
    }

    private func loadChildren() {
        guard isDirectory else { return }

        let fm = FileManager.default
        guard let contents = try? fm.contentsOfDirectory(at: url, includingPropertiesForKeys: [.isDirectoryKey]) else {
            return
        }

        children = contents
            .filter { !$0.lastPathComponent.hasPrefix(".") }
            .sorted { $0.lastPathComponent < $1.lastPathComponent }
    }
}

#Preview {
    let state = AppState()
    return PromptLauncher(appState: state)
        .frame(width: 600, height: 700)
}

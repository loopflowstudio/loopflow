// Main content view for composing and launching lf commands.

import SwiftUI
import UniformTypeIdentifiers

struct PromptLauncher: View {
    @Bindable var appState: AppState

    @State private var inputText: String = ""
    @State private var showingPromptPicker = false
    @State private var highlightedPromptIndex = 0
    @State private var launchError: String?
    @State private var showingLaunchError = false
    @FocusState private var isInputFocused: Bool
    @State private var isCreatingWorktree = false

    private let terminalLauncher = TerminalLauncher()

    // Parse input into prompt name and args
    private var parsedInput: (prompt: String?, args: String) {
        let trimmed = inputText.trimmingCharacters(in: .whitespaces)
        if let colonIndex = trimmed.firstIndex(of: ":") {
            let promptPart = String(trimmed[..<colonIndex]).trimmingCharacters(in: .whitespaces)
            let argsPart = String(trimmed[trimmed.index(after: colonIndex)...]).trimmingCharacters(in: .whitespaces)
            return (promptPart.isEmpty ? nil : promptPart, argsPart)
        }
        // Check if the entire input matches a prompt name
        if appState.prompts.contains(where: { $0.name.lowercased() == trimmed.lowercased() }) {
            return (trimmed, "")
        }
        return (nil, trimmed)
    }

    private var filteredPrompts: [PromptCard] {
        let query = parsedInput.prompt?.lowercased() ?? inputText.lowercased()
        if query.isEmpty { return appState.prompts }
        return appState.prompts.filter { $0.name.lowercased().contains(query) }
    }

    private var selectedPrompt: PromptCard? {
        guard let name = parsedInput.prompt else { return nil }
        return appState.prompts.first { $0.name.lowercased() == name.lowercased() }
    }

    var body: some View {
        VStack(spacing: 0) {
            Spacer()

            // Main input area - centered and prominent
            VStack(spacing: 12) {
                taskSelector
                voiceSelector
                mainInput
                optionsBar
            }
            .frame(maxWidth: 600)
            .padding(.horizontal, 40)

            Spacer()
        }
        .alert("Launch Failed", isPresented: $showingLaunchError) {
            Button("OK") { launchError = nil }
        } message: {
            Text(launchError ?? "Failed to launch terminal")
        }
        .onAppear {
            isInputFocused = true
        }
    }

    // MARK: - Task Selector

    @State private var taskSearchText: String = ""
    @State private var isTaskSearchFocused: Bool = false
    @State private var highlightedTaskIndex: Int = 0
    @FocusState private var taskFieldFocused: Bool

    private var filteredTasks: [PromptCard] {
        if taskSearchText.isEmpty {
            return appState.prompts
        }
        return appState.prompts.filter { $0.name.lowercased().contains(taskSearchText.lowercased()) }
    }

    private var taskSelector: some View {
        HStack(spacing: 8) {
            Text("Task")
                .font(.caption)
                .foregroundStyle(.secondary)

            ZStack(alignment: .topLeading) {
                // Typeahead input
                HStack(spacing: 4) {
                    TextField("None", text: $taskSearchText)
                        .textFieldStyle(.plain)
                        .font(.system(size: 13, weight: .medium))
                        .focused($taskFieldFocused)
                        .onChange(of: taskSearchText) { _, _ in
                            isTaskSearchFocused = true
                            highlightedTaskIndex = 0
                        }
                        .onChange(of: taskFieldFocused) { _, focused in
                            isTaskSearchFocused = focused
                            if focused {
                                taskSearchText = ""
                            }
                        }
                        .onKeyPress(.downArrow) {
                            if isTaskSearchFocused && !filteredTasks.isEmpty {
                                highlightedTaskIndex = min(highlightedTaskIndex + 1, filteredTasks.count - 1)
                                return .handled
                            }
                            return .ignored
                        }
                        .onKeyPress(.upArrow) {
                            if isTaskSearchFocused {
                                highlightedTaskIndex = max(highlightedTaskIndex - 1, 0)
                                return .handled
                            }
                            return .ignored
                        }
                        .onKeyPress(.return) {
                            if isTaskSearchFocused && !filteredTasks.isEmpty {
                                selectTask(filteredTasks[highlightedTaskIndex])
                                return .handled
                            } else if isTaskSearchFocused && taskSearchText.isEmpty {
                                clearTaskSelection()
                                return .handled
                            }
                            return .ignored
                        }
                        .onKeyPress(.escape) {
                            if isTaskSearchFocused {
                                taskFieldFocused = false
                                isTaskSearchFocused = false
                                taskSearchText = selectedPrompt?.displayName ?? ""
                                return .handled
                            }
                            return .ignored
                        }

                    if selectedPrompt != nil {
                        Button {
                            clearTaskSelection()
                        } label: {
                            Image(systemName: "xmark.circle.fill")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 6)
                .background(
                    RoundedRectangle(cornerRadius: 6)
                        .fill(Color.primary.opacity(0.05))
                )
                .frame(minWidth: 120)

                // Dropdown results
                if isTaskSearchFocused && !filteredTasks.isEmpty {
                    VStack(alignment: .leading, spacing: 0) {
                        ForEach(Array(filteredTasks.enumerated()), id: \.element.id) { index, prompt in
                            Button {
                                selectTask(prompt)
                            } label: {
                                HStack {
                                    Text(prompt.displayName)
                                        .fontWeight(.medium)
                                    Spacer()
                                    Text(prompt.defaultMode == .auto ? "auto" : "interactive")
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                .padding(.horizontal, 10)
                                .padding(.vertical, 6)
                                .background(index == highlightedTaskIndex ? Color.accentColor.opacity(0.1) : Color.clear)
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .background(Color(nsColor: .controlBackgroundColor))
                    .clipShape(RoundedRectangle(cornerRadius: 6))
                    .overlay(
                        RoundedRectangle(cornerRadius: 6)
                            .stroke(Color.primary.opacity(0.1), lineWidth: 1)
                    )
                    .shadow(color: .black.opacity(0.1), radius: 4, y: 2)
                    .offset(y: 32)
                    .zIndex(100)
                }
            }

            Spacer()
        }
    }

    private func selectTask(_ prompt: PromptCard) {
        taskSearchText = prompt.displayName
        isTaskSearchFocused = false
        taskFieldFocused = false
        selectPromptFromMenu(prompt)
    }

    private func clearTaskSelection() {
        taskSearchText = ""
        isTaskSearchFocused = false
        taskFieldFocused = false
        clearPromptSelection()
    }

    private func selectPromptFromMenu(_ prompt: PromptCard) {
        // Update input to have prompt prefix
        let currentArgs = parsedInput.args
        inputText = "\(prompt.name): \(currentArgs)"
        appState.runMode = prompt.defaultMode
    }

    private func clearPromptSelection() {
        // Keep just the args part
        inputText = parsedInput.args
    }

    // MARK: - Voice Selector

    @State private var showingVoicePicker = false

    private var voiceSelector: some View {
        HStack(spacing: 8) {
            Text("Voice")
                .font(.caption)
                .foregroundStyle(.secondary)

            // Selected voice chips
            if appState.selectedVoices.isEmpty {
                Button {
                    showingVoicePicker = true
                } label: {
                    Text("None")
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 6)
                        .background(
                            RoundedRectangle(cornerRadius: 6)
                                .fill(Color.primary.opacity(0.05))
                        )
                }
                .buttonStyle(.plain)
                .popover(isPresented: $showingVoicePicker) {
                    voicePickerPopover()
                }
            } else {
                HStack(spacing: 6) {
                    ForEach(appState.selectedVoices) { voice in
                        voiceChip(voice)
                    }

                    if appState.voices.count > appState.selectedVoices.count {
                        Button {
                            showingVoicePicker = true
                        } label: {
                            Image(systemName: "plus")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .padding(6)
                                .background(Circle().fill(Color.primary.opacity(0.05)))
                        }
                        .buttonStyle(.plain)
                        .popover(isPresented: $showingVoicePicker) {
                            voicePickerPopover()
                        }
                    }
                }
            }

            Spacer()
        }
    }

    private func voiceChip(_ voice: Voice) -> some View {
        HStack(spacing: 4) {
            Text(voice.displayName)
                .font(.caption)
                .fontWeight(.medium)
            Button {
                appState.selectedVoices.removeAll { $0.id == voice.id }
            } label: {
                Image(systemName: "xmark")
                    .font(.caption2)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(Capsule().fill(Color.purple.opacity(0.15)))
        .foregroundStyle(Color.purple)
    }

    private func voicePickerPopover() -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Available Voices")
                .font(.caption)
                .foregroundStyle(.secondary)
                .padding(.horizontal, 12)
                .padding(.top, 12)
                .padding(.bottom, 8)

            if appState.voices.isEmpty {
                Text("No voices in .lf/voices/")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
                    .padding(.horizontal, 12)
                    .padding(.bottom, 12)
            } else {
                ForEach(appState.voices) { voice in
                    let isSelected = appState.selectedVoices.contains(voice)
                    Button {
                        if isSelected {
                            appState.selectedVoices.removeAll { $0.id == voice.id }
                        } else {
                            appState.selectedVoices.append(voice)
                        }
                    } label: {
                        HStack {
                            Image(systemName: isSelected ? "checkmark.circle.fill" : "circle")
                                .foregroundStyle(isSelected ? Color.purple : Color.secondary)
                            Text(voice.displayName)
                                .fontWeight(.medium)
                            Spacer()
                        }
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }
        }
        .frame(minWidth: 200)
    }

    // MARK: - Main Input

    private var mainInput: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Prompt picker dropdown
            if showingPromptPicker && !filteredPrompts.isEmpty {
                promptPicker
            }

            // Text input
            ZStack(alignment: .topLeading) {
                // Placeholder
                if inputText.isEmpty {
                    Text("What do you want to build?")
                        .foregroundStyle(.tertiary)
                        .padding(.horizontal, 4)
                        .padding(.vertical, 12)
                }

                TextEditor(text: $inputText)
                    .font(.title3)
                    .scrollContentBackground(.hidden)
                    .focused($isInputFocused)
                    .frame(minHeight: 80, maxHeight: 200)
                    .onChange(of: inputText) { _, newValue in
                        // Show picker when typing potential prompt name (before colon)
                        let hasColon = newValue.contains(":")
                        let isTypingPrompt = !hasColon && !newValue.isEmpty
                        showingPromptPicker = isTypingPrompt && !filteredPrompts.isEmpty
                        highlightedPromptIndex = 0
                    }
                    .onKeyPress(.downArrow) {
                        if showingPromptPicker {
                            highlightedPromptIndex = min(highlightedPromptIndex + 1, filteredPrompts.count - 1)
                            return .handled
                        }
                        return .ignored
                    }
                    .onKeyPress(.upArrow) {
                        if showingPromptPicker {
                            highlightedPromptIndex = max(highlightedPromptIndex - 1, 0)
                            return .handled
                        }
                        return .ignored
                    }
                    .onKeyPress(.tab) {
                        if showingPromptPicker, !filteredPrompts.isEmpty {
                            selectPrompt(filteredPrompts[highlightedPromptIndex])
                            return .handled
                        }
                        return .ignored
                    }
                    .onKeyPress(.return) {
                        if showingPromptPicker, !filteredPrompts.isEmpty {
                            selectPrompt(filteredPrompts[highlightedPromptIndex])
                            return .handled
                        }
                        return .ignored
                    }
            }
            .padding(16)

            // Bottom bar with run button
            HStack {
                // Selected prompt badge
                if let prompt = selectedPrompt {
                    HStack(spacing: 4) {
                        Text(prompt.displayName)
                            .font(.caption)
                            .fontWeight(.medium)
                        Button {
                            // Clear the prompt prefix
                            if let colonIndex = inputText.firstIndex(of: ":") {
                                inputText = String(inputText[inputText.index(after: colonIndex)...]).trimmingCharacters(in: .whitespaces)
                            }
                        } label: {
                            Image(systemName: "xmark")
                                .font(.caption2)
                        }
                        .buttonStyle(.plain)
                    }
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(Capsule().fill(Color.accentColor.opacity(0.15)))
                    .foregroundStyle(Color.accentColor)
                }

                Spacer()

                // Run button
                Button {
                    launchInTerminal()
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "play.fill")
                            .font(.caption)
                        Text("Run")
                        Text("⌘↵")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                }
                .buttonStyle(.borderedProminent)
                .keyboardShortcut(.return, modifiers: .command)
            }
            .padding(.horizontal, 16)
            .padding(.bottom, 12)
        }
        .background(
            RoundedRectangle(cornerRadius: 16)
                .fill(.background)
                .shadow(color: .black.opacity(0.08), radius: 8, y: 2)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 16)
                .stroke(Color.primary.opacity(0.1), lineWidth: 1)
        )
    }

    // MARK: - Prompt Picker

    private var promptPicker: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(filteredPrompts.enumerated()), id: \.element.id) { index, prompt in
                Button {
                    selectPrompt(prompt)
                } label: {
                    HStack {
                        Text(prompt.displayName)
                            .fontWeight(.medium)
                        Spacer()
                        Text(prompt.defaultMode == .auto ? "auto" : "interactive")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .background(index == highlightedPromptIndex ? Color.accentColor.opacity(0.1) : Color.clear)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.vertical, 4)
        .background(Color(nsColor: .controlBackgroundColor))
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(Color.primary.opacity(0.1), lineWidth: 1)
        )
        .padding(.horizontal, 8)
        .padding(.bottom, 8)
    }

    private func selectPrompt(_ prompt: PromptCard) {
        inputText = "\(prompt.name): "
        showingPromptPicker = false
        appState.runMode = prompt.defaultMode
    }

    // MARK: - Context Bar (always visible)

    @State private var isDraggingOver = false
    @State private var showingFilePicker = false

    private var optionsBar: some View {
        VStack(alignment: .leading, spacing: 10) {
            // Mode picker row
            HStack(spacing: 12) {
                Picker("", selection: $appState.runMode) {
                    Text("Auto").tag(RunMode.auto)
                    Text("Interactive").tag(RunMode.interactive)
                }
                .pickerStyle(.segmented)
                .frame(width: 160)

                Spacer()
            }

            // Context bar - always visible
            contextBar
        }
        .onChange(of: appState.includeDocs) {
            Task { await appState.estimateTokens() }
        }
        .onChange(of: appState.includeDiff) {
            Task { await appState.estimateTokens() }
        }
        .onChange(of: appState.includeDiffFiles) {
            Task { await appState.estimateTokens() }
        }
        .onChange(of: appState.includePaste) {
            Task { await appState.estimateTokens() }
        }
        .onChange(of: appState.attachedFiles) {
            Task { await appState.estimateTokens() }
        }
    }

    private var contextBar: some View {
        HStack(spacing: 6) {
            // Context chips
            ContextChip(label: "Docs", isOn: $appState.includeDocs, color: .blue)
                .help("Include README, STYLE, and other docs")
            ContextChip(label: "Files", isOn: $appState.includeDiffFiles, color: .teal)
                .help("Include files touched by this branch")
            ContextChip(label: "Diff", isOn: $appState.includeDiff, color: .green)
                .help("Include raw diff output")
            ContextChip(label: "Clipboard", isOn: $appState.includePaste, color: .purple)
                .help("Include clipboard content")

            // Separator if there are attached files
            if !appState.attachedFiles.isEmpty {
                Divider()
                    .frame(height: 16)
                    .padding(.horizontal, 2)
            }

            // Attached file chips
            ForEach(appState.attachedFiles, id: \.self) { file in
                FileChip(
                    name: file.lastPathComponent,
                    icon: iconForFile(file)
                ) {
                    appState.attachedFiles.removeAll { $0 == file }
                }
            }

            // Add file button
            AddFileButton {
                showingFilePicker = true
            }
            .fileImporter(
                isPresented: $showingFilePicker,
                allowedContentTypes: [.item],
                allowsMultipleSelection: true
            ) { result in
                if case .success(let urls) = result {
                    for url in urls {
                        if !appState.attachedFiles.contains(url) {
                            appState.attachedFiles.append(url)
                        }
                    }
                }
            }

            Spacer()

            // Token count
            Text(formatTokenCount(appState.estimatedTokens))
                .font(.system(size: 12, weight: .medium, design: .monospaced))
                .foregroundStyle(.secondary)
        }
        .padding(.vertical, 8)
        .padding(.horizontal, 12)
        .background(
            RoundedRectangle(cornerRadius: 10)
                .fill(Color.primary.opacity(0.03))
        )
        .onDrop(of: [.fileURL], isTargeted: $isDraggingOver) { providers in
            handleFileDrop(providers)
        }
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(isDraggingOver ? Color.accentColor : Color.clear, lineWidth: 2)
        )
    }

    private func formatTokenCount(_ count: Int) -> String {
        if count >= 1000 {
            let k = Double(count) / 1000.0
            return String(format: "%.1fk", k)
        }
        return "\(count)"
    }

    private func iconForFile(_ file: URL) -> String {
        let ext = file.pathExtension.lowercased()
        switch ext {
        case "png", "jpg", "jpeg", "gif", "webp", "heic":
            return "photo"
        case "pdf":
            return "doc.richtext"
        case "swift", "py", "js", "ts", "rs", "go":
            return "chevron.left.forwardslash.chevron.right"
        case "md", "txt":
            return "doc.text"
        default:
            return "doc"
        }
    }

    private func handleFileDrop(_ providers: [NSItemProvider]) -> Bool {
        for provider in providers {
            if provider.hasItemConformingToTypeIdentifier("public.file-url") {
                _ = provider.loadObject(ofClass: URL.self) { url, _ in
                    if let url = url {
                        DispatchQueue.main.async {
                            if !appState.attachedFiles.contains(url) {
                                appState.attachedFiles.append(url)
                            }
                        }
                    }
                }
            }
        }
        return true
    }

    // MARK: - Actions

    private func launchInTerminal() {
        guard let repo = appState.currentRepo else { return }

        // Update appState from parsed input
        appState.selectedPrompt = selectedPrompt
        appState.promptArgs = parsedInput.args

        // Check if on main branch - if so, create new worktree first
        let isMain = appState.selectedWorktree?.branch == "main"
                  || appState.selectedWorktree == nil

        if isMain {
            isCreatingWorktree = true
            Task {
                await launchWithNewWorktree(repo: repo)
                isCreatingWorktree = false
            }
        } else {
            launchCommand(repo: repo, workPath: URL(fileURLWithPath: appState.selectedWorktree!.path))
        }
    }

    private func launchWithNewWorktree(repo: URL) async {
        let name = NameGenerator.generate()

        do {
            try await appState.createWorktree(name: name)
            await appState.refreshWorktrees()

            // Select the newly created worktree
            if let newWorktree = appState.worktrees.first(where: { $0.branch == name }) {
                appState.selectedWorktree = newWorktree
                let workPath = URL(fileURLWithPath: newWorktree.path)
                launchCommand(repo: repo, workPath: workPath)
            } else {
                launchError = "Failed to find newly created worktree"
                showingLaunchError = true
            }
        } catch {
            launchError = "Failed to create worktree: \(error.localizedDescription)"
            showingLaunchError = true
        }
    }

    private func launchCommand(repo: URL, workPath: URL) {
        let terminal = appState.config?.terminalApp ?? .warp
        let command = appState.buildCommand()

        do {
            try terminalLauncher.launchTerminal(terminal, at: workPath, command: command)
        } catch {
            launchError = error.localizedDescription
            showingLaunchError = true
        }
    }
}

// MARK: - Context Picker

struct ContextPicker: View {
    let repoURL: URL
    let excludePatterns: [String]
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
                // Filter hidden files
                guard !name.hasPrefix(".") else { return false }
                // Check exclude patterns from config
                for pattern in excludePatterns {
                    if name == pattern || pattern.hasPrefix(name + "/") {
                        return false
                    }
                }
                return true
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

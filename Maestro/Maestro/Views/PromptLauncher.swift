// Main content view for composing and launching lf commands.

import SwiftUI
import UniformTypeIdentifiers

struct PromptLauncher: View {
    @Bindable var appState: AppState

    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var inputText: String = ""
    @State private var showingPromptPicker = false
    @State private var highlightedPromptIndex = 0
    @State private var launchError: String?
    @State private var showingLaunchError = false
    @FocusState private var isInputFocused: Bool
    @State private var isCreatingWorktree = false
    @State private var showAdvancedOptions = false
    @State private var isPreviewExpanded = false
    @State private var expandedSections: Set<ContextKind> = [.docs, .files]

    private let terminalLauncher = TerminalLauncher()
    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

    // Track whether a pipeline is selected (vs a task)
    @State private var selectedPipeline: PipelineDef?

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
                mainInput
                modeAndAdvancedRow
                if showAdvancedOptions {
                    modelSelector
                    voiceSelector
                    contextBar
                    commandPreview
                }
            }
            .frame(maxWidth: 600)
            .padding(.horizontal, 40)

            Spacer()
        }
        .alert("Couldn't Start", isPresented: $showingLaunchError) {
            Button("OK") { launchError = nil }
        } message: {
            Text(launchError ?? "Something went wrong. Try again.")
        }
        .onAppear {
            isInputFocused = true
        }
        .onChange(of: appState.prompts) { _, prompts in
            // Auto-select "implement" task on first load if available
            if !hasInitializedTask && !prompts.isEmpty {
                hasInitializedTask = true
                if let implementTask = prompts.first(where: { $0.name.lowercased() == "implement" }) {
                    selectTask(implementTask)
                }
            }
        }
        .background {
            // Hidden button for Cmd+L keyboard shortcut to focus prompt
            Button("") {
                isInputFocused = true
            }
            .keyboardShortcut("l", modifiers: .command)
            .opacity(0)
        }
    }

    // MARK: - Task Selector

    @State private var taskSearchText: String = "implement"
    @State private var isTaskSearchFocused: Bool = false
    @State private var highlightedTaskIndex: Int = 0
    @FocusState private var taskFieldFocused: Bool
    @State private var hasInitializedTask: Bool = false

    private var filteredTasks: [PromptCard] {
        if taskSearchText.isEmpty {
            return appState.prompts
        }
        return appState.prompts.filter { $0.name.lowercased().contains(taskSearchText.lowercased()) }
    }

    private var filteredPipelines: [PipelineDef] {
        guard Flags.beta else { return [] }
        if taskSearchText.isEmpty {
            return appState.pipelines
        }
        return appState.pipelines.filter { $0.name.lowercased().contains(taskSearchText.lowercased()) }
    }

    private var taskSelector: some View {
        HStack(spacing: 8) {
            Text("Task")
                .font(.caption)
                .foregroundStyle(.secondary)

            ZStack(alignment: .topLeading) {
                // Typeahead input
                HStack(spacing: 4) {
                    TextField("Select task...", text: $taskSearchText)
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
                        .fill(palette.surfaceMuted)
                )
                .frame(minWidth: 120)

                // Dropdown results
                if isTaskSearchFocused && (!filteredTasks.isEmpty || !filteredPipelines.isEmpty) {
                    VStack(alignment: .leading, spacing: 0) {
                        // Tasks section (local prompts)
                        let localTasks = filteredTasks.filter { !$0.isExternalSkill }
                        if !localTasks.isEmpty {
                            Text("Tasks")
                                .font(.caption2)
                                .foregroundStyle(.tertiary)
                                .padding(.horizontal, 10)
                                .padding(.top, 6)
                                .padding(.bottom, 2)

                            ForEach(Array(localTasks.enumerated()), id: \.element.id) { index, prompt in
                                Button {
                                    selectTask(prompt)
                                } label: {
                                    VStack(alignment: .leading, spacing: 3) {
                                        HStack {
                                            Text(prompt.displayName)
                                                .fontWeight(.medium)
                                            Spacer()
                                            Text(prompt.defaultMode == .auto ? "auto" : "interactive")
                                                .font(.caption)
                                                .foregroundStyle(.secondary)
                                        }
                                        if let description = prompt.shortDescription {
                                            Text(description)
                                                .font(.caption)
                                                .foregroundStyle(.secondary)
                                                .lineLimit(2)
                                                .fixedSize(horizontal: false, vertical: true)
                                        }
                                    }
                                    .padding(.horizontal, 10)
                                    .padding(.vertical, 8)
                                    .background(index == highlightedTaskIndex ? Color.accentColor.opacity(0.1) : Color.clear)
                                }
                                .buttonStyle(.plain)
                            }
                        }

                        // External skills section
                        let externalSkills = filteredTasks.filter { $0.isExternalSkill }
                        if !externalSkills.isEmpty {
                            // Group by source
                            let bySource = Dictionary(grouping: externalSkills) { $0.source.sourceName ?? "external" }
                            ForEach(Array(bySource.keys.sorted()), id: \.self) { sourceName in
                                Text(sourceName.capitalized)
                                    .font(.caption2)
                                    .foregroundStyle(.tertiary)
                                    .padding(.horizontal, 10)
                                    .padding(.top, 6)
                                    .padding(.bottom, 2)

                                ForEach(Array((bySource[sourceName] ?? []).enumerated()), id: \.element.id) { index, prompt in
                                    let adjustedIndex = localTasks.count + index
                                    Button {
                                        selectTask(prompt)
                                    } label: {
                                        HStack {
                                            Image(systemName: "link")
                                                .font(.caption2)
                                                .foregroundStyle(.secondary)
                                            Text(prompt.displayName)
                                                .fontWeight(.medium)
                                            Spacer()
                                            Text("auto")
                                                .font(.caption)
                                                .foregroundStyle(.secondary)
                                        }
                                        .padding(.horizontal, 10)
                                        .padding(.vertical, 6)
                                        .background(adjustedIndex == highlightedTaskIndex ? Color.accentColor.opacity(0.1) : Color.clear)
                                    }
                                    .buttonStyle(.plain)
                                }
                            }
                        }

                        // Pipelines section
                        if !filteredPipelines.isEmpty {
                            Text("Pipelines")
                                .font(.caption2)
                                .foregroundStyle(.tertiary)
                                .padding(.horizontal, 10)
                                .padding(.top, 6)
                                .padding(.bottom, 2)

                            ForEach(Array(filteredPipelines.enumerated()), id: \.element.id) { index, pipeline in
                                let adjustedIndex = filteredTasks.count + index
                                Button {
                                    selectPipelineFromDropdown(pipeline)
                                } label: {
                                    HStack {
                                        Image(systemName: "arrow.triangle.branch")
                                            .font(.caption)
                                            .foregroundStyle(.secondary)
                                        Text(pipeline.name)
                                            .fontWeight(.medium)
                                        Spacer()
                                        Text("\(pipeline.stepCount) steps")
                                            .font(.caption)
                                            .foregroundStyle(.secondary)
                                    }
                                    .padding(.horizontal, 10)
                                    .padding(.vertical, 6)
                                    .background(adjustedIndex == highlightedTaskIndex ? Color.accentColor.opacity(0.1) : Color.clear)
                                }
                                .buttonStyle(.plain)
                            }
                        }
                    }
                    .background(palette.surface)
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
        selectedPipeline = nil  // Clear pipeline selection
        selectPromptFromMenu(prompt)
    }

    private func selectPipelineFromDropdown(_ pipeline: PipelineDef) {
        taskSearchText = pipeline.name
        isTaskSearchFocused = false
        taskFieldFocused = false
        selectedPipeline = pipeline
        // Clear task input when selecting a pipeline
        inputText = ""
        appState.runMode = .auto  // Pipelines always run in auto mode
    }

    private func clearTaskSelection() {
        taskSearchText = ""
        isTaskSearchFocused = false
        taskFieldFocused = false
        selectedPipeline = nil
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

    // MARK: - Model Selector

    @State private var showingModelPicker = false

    private var modelSelector: some View {
        HStack(spacing: 8) {
            Text("Model")
                .font(.caption)
                .foregroundStyle(.secondary)

            Button {
                showingModelPicker = true
            } label: {
                HStack(spacing: 4) {
                    Text(modelDisplayText)
                        .font(.system(size: 13, weight: .medium))
                    Image(systemName: "chevron.down")
                        .font(.caption2)
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 6)
                .background(
                    RoundedRectangle(cornerRadius: 6)
                        .fill(Color.primary.opacity(0.05))
                )
            }
            .buttonStyle(.plain)
            .popover(isPresented: $showingModelPicker) {
                modelPickerPopover()
            }

            Spacer()
        }
    }

    private var modelDisplayText: String {
        if let model = appState.selectedModel {
            return model.displayName
        }
        if let configModel = appState.config?.agentModel {
            return AgentModel(configModel).displayName + " (default)"
        }
        return "Default"
    }

    private func modelPickerPopover() -> some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Select Model")
                .font(.caption)
                .foregroundStyle(.secondary)
                .padding(.horizontal, 12)
                .padding(.top, 12)
                .padding(.bottom, 8)

            // Default option (uses config)
            Button {
                appState.selectedModel = nil
                showingModelPicker = false
            } label: {
                HStack {
                    Image(systemName: appState.selectedModel == nil ? "checkmark.circle.fill" : "circle")
                        .foregroundStyle(appState.selectedModel == nil ? Color.accentColor : Color.secondary)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Default")
                            .fontWeight(.medium)
                        if let configModel = appState.config?.agentModel {
                            Text(AgentModel(configModel).displayName)
                                .font(.caption2)
                                .foregroundStyle(.tertiary)
                        }
                    }
                    Spacer()
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            Divider()
                .padding(.horizontal, 12)
                .padding(.vertical, 4)

            // Common models
            ForEach(AgentModel.commonModels) { model in
                let isSelected = appState.selectedModel?.id == model.id
                Button {
                    appState.selectedModel = model
                    showingModelPicker = false
                } label: {
                    HStack {
                        Image(systemName: isSelected ? "checkmark.circle.fill" : "circle")
                            .foregroundStyle(isSelected ? Color.accentColor : Color.secondary)
                        Text(model.displayName)
                            .fontWeight(.medium)
                        Spacer()
                        Text(model.cliValue)
                            .font(.caption)
                            .foregroundStyle(.tertiary)
                    }
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .frame(minWidth: 220)
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
        .background(Capsule().fill(Color.accentColor.opacity(0.15)))
        .foregroundStyle(Color.accentColor)
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
                VStack(alignment: .leading, spacing: 4) {
                    Text("No voices configured")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Text("Add .md files to .lf/voices/ to create personas that shape how the agent responds.")
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .padding(.horizontal, 12)
                .padding(.bottom, 12)
                .frame(maxWidth: 220)
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
                                .foregroundStyle(isSelected ? Color.accentColor : Color.secondary)
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
                // Placeholder with examples
                if inputText.isEmpty {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("What should the AI build?")
                            .foregroundStyle(.secondary)
                        Text("Try: \"add user authentication\" or \"refactor the API to use async/await\"")
                            .font(.caption)
                            .foregroundStyle(.tertiary)
                    }
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
                .accessibleButton("Run \(taskSearchText.isEmpty ? "task" : taskSearchText)", hint: "Press Command Return to run")
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
        .background(palette.surface)
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

    // MARK: - Mode and Advanced Options Row

    private var modeAndAdvancedRow: some View {
        VStack(spacing: 8) {
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Picker("", selection: $appState.runMode) {
                        Text("Auto").tag(RunMode.auto)
                        Text("Interactive").tag(RunMode.interactive)
                    }
                    .pickerStyle(.segmented)
                    .frame(width: 160)

                    // Inline description of current mode
                    Text(appState.runMode == .auto
                        ? "Runs to completion"
                        : "Chat with the AI")
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                }
                .help(appState.runMode == .auto
                    ? "Auto: Runs to completion without interruption. Best for most tasks."
                    : "Interactive: Chat with the AI, can interrupt and redirect. Use for exploration.")

                Spacer()

                // Token count - clickable to expand preview
                Button {
                    withAnimation(DesignAnimation.standard(reduceMotion)) {
                        isPreviewExpanded.toggle()
                        if isPreviewExpanded {
                            Task { await appState.refreshContextPreview() }
                        }
                    }
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: isPreviewExpanded ? "chevron.down" : "chevron.right")
                            .font(.caption2)
                        Text(formatTokenCount(appState.estimatedTokens))
                            .font(.system(size: 12, weight: .medium, design: .monospaced))
                    }
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(
                        RoundedRectangle(cornerRadius: 6)
                            .fill(isPreviewExpanded ? Color.primary.opacity(0.08) : Color.primary.opacity(0.03))
                    )
                }
                .buttonStyle(.plain)
                .help("Click to see context breakdown")
                .accessibleButton("Toggle context preview", hint: isPreviewExpanded ? "Collapse" : "Expand")

                Button {
                    withAnimation(DesignAnimation.standard(reduceMotion)) {
                        showAdvancedOptions.toggle()
                    }
                } label: {
                    HStack(spacing: 4) {
                        Text(showAdvancedOptions ? "Options" : "More options")
                            .font(.caption)
                        Image(systemName: showAdvancedOptions ? "chevron.up" : "chevron.down")
                            .font(.caption2)
                    }
                    .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
                .help("Model, voice, context toggles, and command preview")
                .accessibleButton("Toggle advanced options", hint: showAdvancedOptions ? "Collapse" : "Expand")
            }

            // Context preview panel
            if isPreviewExpanded {
                contextPreviewPanel
            }
        }
    }

    // MARK: - Context Preview Panel

    private var contextPreviewPanel: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Header
            HStack {
                Text("Context Preview")
                    .font(.caption)
                    .fontWeight(.medium)
                    .foregroundStyle(.secondary)

                Spacer()

                Button {
                    Task {
                        if let _ = await appState.copyAssembledContext() {
                            // Could show a confirmation toast
                        }
                    }
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "doc.on.doc")
                            .font(.caption2)
                        Text("Copy")
                            .font(.caption)
                    }
                    .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
                .help("Copy full assembled context to clipboard")
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)

            Divider()

            // Sections
            ScrollView {
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(appState.contextPreview.sections) { section in
                        contextSectionView(section)
                    }

                    if appState.contextPreview.sections.isEmpty {
                        HStack {
                            Spacer()
                            Text("Loading context...")
                                .font(.caption)
                                .foregroundStyle(.tertiary)
                            Spacer()
                        }
                        .padding(.vertical, 20)
                    }
                }
                .padding(8)
            }
            .frame(maxHeight: 250)
        }
        .background(
            RoundedRectangle(cornerRadius: 10)
                .fill(palette.surface)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(Color.primary.opacity(0.1), lineWidth: 1)
        )
    }

    private func contextSectionView(_ section: ContextSection) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            // Section header - clickable to expand/collapse
            Button {
                withAnimation(DesignAnimation.fast(reduceMotion)) {
                    if expandedSections.contains(section.kind) {
                        expandedSections.remove(section.kind)
                    } else {
                        expandedSections.insert(section.kind)
                    }
                }
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: expandedSections.contains(section.kind) ? "chevron.down" : "chevron.right")
                        .font(.caption2)
                        .frame(width: 10)

                    Image(systemName: section.kind.icon)
                        .font(.caption)
                        .foregroundStyle(section.isEnabled ? section.kind.color : .secondary)

                    Text(section.kind.displayName)
                        .font(.caption)
                        .fontWeight(.medium)
                        .foregroundStyle(section.isEnabled ? .primary : .secondary)

                    Text("(\(formatTokenCount(section.tokens)))")
                        .font(.caption)
                        .foregroundStyle(.tertiary)

                    Spacer()
                }
                .padding(.vertical, 4)
                .padding(.horizontal, 4)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .opacity(section.isEnabled ? 1.0 : 0.5)

            // Section items (when expanded)
            if expandedSections.contains(section.kind) && section.isEnabled {
                ForEach(section.items) { item in
                    contextItemView(item, section: section)
                }
            }
        }
    }

    private func contextItemView(_ item: ContextItem, section: ContextSection) -> some View {
        HStack(spacing: 6) {
            Spacer()
                .frame(width: 16)

            Image(systemName: "doc")
                .font(.caption2)
                .foregroundStyle(.tertiary)

            Text(item.name)
                .font(.caption)
                .lineLimit(1)
                .truncationMode(.middle)

            Spacer()

            // Remove button for files and attached
            if section.kind == .files || section.kind == .attached {
                Button {
                    appState.removeContextItem(item, from: section)
                } label: {
                    Image(systemName: "xmark")
                        .font(.system(size: 9, weight: .medium))
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
                .help("Remove from context")
            }

            Text(formatTokenCount(item.tokens))
                .font(.caption)
                .foregroundStyle(.tertiary)
                .frame(width: 40, alignment: .trailing)
        }
        .padding(.vertical, 2)
        .padding(.horizontal, 4)
        .background(
            RoundedRectangle(cornerRadius: 4)
                .fill(Color.primary.opacity(0.02))
        )
    }

    // MARK: - Context Bar

    @State private var isDraggingOver = false
    @State private var showingFilePicker = false
    @AppStorage("contextBarExpanded") private var contextBarExpanded = false

    private var contextBar: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Collapsible header - always visible
            Button {
                withAnimation(DesignAnimation.fast(reduceMotion)) {
                    contextBarExpanded.toggle()
                }
            } label: {
                HStack(spacing: 6) {
                    Text("Context")
                        .font(.caption)
                        .fontWeight(.medium)
                    Text(formatTokenCount(appState.estimatedTokens))
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                    Spacer()
                    Image(systemName: contextBarExpanded ? "chevron.up" : "chevron.down")
                        .font(.caption2)
                }
                .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .accessibleButton("Toggle context options", hint: contextBarExpanded ? "Collapse" : "Expand")

            // Expanded content
            if contextBarExpanded {
                HStack(spacing: 6) {
                    // Context chips
                    ContextChip(label: "Docs", isOn: $appState.includeDocs, color: .blue)
                        .help("AI sees: README, STYLE guide, and project docs. Helps it follow your conventions.")
                    ContextChip(label: "Files", isOn: $appState.includeDiffFiles, color: .teal)
                        .help("AI sees: Files you've changed on this branch. Helps it understand your work in progress.")
                    ContextChip(label: "Diff", isOn: $appState.includeDiff, color: .green)
                        .help("AI sees: Exact line-by-line changes. Useful for reviews and understanding what changed.")
                    ContextChip(label: "Clipboard", isOn: $appState.includePaste, color: .accentColor)
                        .help("AI sees: Whatever you copied. Paste code, errors, or docs for reference.")
                    ContextChip(label: "Chrome", isOn: $appState.includeChrome, color: .indigo)
                        .help("AI can control Chrome browser for web testing and automation.")
                    if appState.config?.hasSummaries == true {
                        ContextChip(label: "Summaries", isOn: $appState.includeSummaries, color: .orange)
                            .help("AI sees: Architecture overview of your codebase. Pre-generated for faster responses.")
                    }
                }

                // Attached files row
                HStack(spacing: 6) {
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
                }
            }
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
        .onChange(of: appState.includeDocs) {
            Task {
                await appState.estimateTokens()
                if isPreviewExpanded { await appState.refreshContextPreview() }
            }
        }
        .onChange(of: appState.includeDiff) {
            Task {
                await appState.estimateTokens()
                if isPreviewExpanded { await appState.refreshContextPreview() }
            }
        }
        .onChange(of: appState.includeDiffFiles) {
            Task {
                await appState.estimateTokens()
                if isPreviewExpanded { await appState.refreshContextPreview() }
            }
        }
        .onChange(of: appState.includePaste) {
            Task {
                await appState.estimateTokens()
                if isPreviewExpanded { await appState.refreshContextPreview() }
            }
        }
        .onChange(of: appState.attachedFiles) {
            Task {
                await appState.estimateTokens()
                if isPreviewExpanded { await appState.refreshContextPreview() }
            }
        }
        .onChange(of: appState.includeSummaries) {
            Task {
                await appState.estimateTokens()
                if isPreviewExpanded { await appState.refreshContextPreview() }
            }
        }
    }

    // MARK: - Command Preview

    @AppStorage("showCommandPreview") private var showCommandPreview = false

    private var commandPreview: some View {
        VStack(alignment: .leading, spacing: 8) {
            Button {
                withAnimation(DesignAnimation.fast(reduceMotion)) {
                    showCommandPreview.toggle()
                }
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: "terminal")
                        .font(.caption)
                    Text("Command Preview")
                        .font(.caption)
                    Spacer()
                    Image(systemName: showCommandPreview ? "chevron.up" : "chevron.down")
                        .font(.caption2)
                }
                .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .accessibleButton("Toggle command preview", hint: showCommandPreview ? "Collapse" : "Expand")

            if showCommandPreview {
                HStack {
                    Text(previewCommand)
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(.primary)
                        .textSelection(.enabled)
                        .lineLimit(3)
                    Spacer()
                    Button {
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString(previewCommand, forType: .string)
                    } label: {
                        Image(systemName: "doc.on.doc")
                            .font(.caption)
                    }
                    .buttonStyle(.plain)
                    .help("Copy command to clipboard")
                }
                .padding(10)
                .background(
                    RoundedRectangle(cornerRadius: 6)
                        .fill(Color.primary.opacity(0.05))
                )
            }
        }
    }

    private var previewCommand: String {
        // Build command from current state
        appState.selectedPrompt = selectedPrompt
        appState.promptArgs = parsedInput.args
        return appState.buildCommand(pipeline: selectedPipeline)
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

            // Select the newly created worktree
            if let newWorktree = appState.worktrees.first(where: { $0.branch == name }) {
                appState.selectedWorktree = newWorktree
                let workPath = URL(fileURLWithPath: newWorktree.path)
                launchCommand(repo: repo, workPath: workPath)
            } else {
                launchError = "Created the branch but couldn't find it. Try refreshing the sidebar."
                showingLaunchError = true
            }
        } catch {
            launchError = error.localizedDescription
            showingLaunchError = true
        }
    }

    private func launchCommand(repo: URL, workPath: URL) {
        let command = appState.buildCommand(pipeline: selectedPipeline)

        // Launch in external terminal
        let terminal = appState.config?.terminalApp ?? .warp
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

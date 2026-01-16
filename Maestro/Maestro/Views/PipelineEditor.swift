// Visual editor for pipeline definitions.

import SwiftUI

struct PipelineEditor: View {
    @Binding var pipeline: PipelineDef
    let availableTasks: [PromptCard]
    let availablePipelines: [PipelineDef]
    let onSave: () -> Void
    let onRun: () -> Void
    let onDelete: () -> Void

    @State private var selectedStepIndex: Int?
    @State private var showingStepEditor = false

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            // Pipeline name
            HStack {
                TextField("Pipeline name", text: $pipeline.name)
                    .textFieldStyle(.roundedBorder)
                    .frame(maxWidth: 200)

                Spacer()

                Button(role: .destructive) {
                    onDelete()
                } label: {
                    Image(systemName: "trash")
                }
                .buttonStyle(.plain)
                .foregroundStyle(.red)
                .help("Delete pipeline")
            }

            // Steps flow (horizontal scroll)
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(Array(pipeline.steps.enumerated()), id: \.element.id) { index, step in
                        StepChip(
                            step: step,
                            isSelected: selectedStepIndex == index
                        ) {
                            selectedStepIndex = index
                            showingStepEditor = true
                        }

                        if index < pipeline.steps.count - 1 {
                            Image(systemName: "arrow.right")
                                .font(.caption)
                                .foregroundStyle(.tertiary)
                        }
                    }

                    AddStepButton {
                        let newStep = PipelineStep(task: availableTasks.first?.name ?? "task")
                        pipeline.steps.append(newStep)
                        selectedStepIndex = pipeline.steps.count - 1
                        showingStepEditor = true
                    }
                }
                .padding(.vertical, 8)
            }

            // Actions
            HStack {
                Button("Save") {
                    onSave()
                }
                .keyboardShortcut("s", modifiers: .command)

                Button("Run") {
                    onRun()
                }
                .buttonStyle(.borderedProminent)
                .disabled(pipeline.steps.isEmpty)
            }
        }
        .padding()
        .sheet(isPresented: $showingStepEditor) {
            if let index = selectedStepIndex, index < pipeline.steps.count {
                StepEditorSheet(
                    step: $pipeline.steps[index],
                    availableTasks: availableTasks,
                    availablePipelines: availablePipelines,
                    onDelete: {
                        pipeline.steps.remove(at: index)
                        selectedStepIndex = nil
                        showingStepEditor = false
                    }
                )
            }
        }
    }
}

// MARK: - Step Chip

struct StepChip: View {
    let step: PipelineStep
    let isSelected: Bool
    let onTap: () -> Void

    var body: some View {
        Button {
            onTap()
        } label: {
            HStack(spacing: 4) {
                if step.pipeline != nil {
                    Image(systemName: "rectangle.stack")
                        .font(.caption2)
                }

                Text(step.displayName)
                    .font(.system(size: 13, weight: .medium))

                if step.hasConfig {
                    Circle()
                        .fill(.blue)
                        .frame(width: 4, height: 4)
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(
                RoundedRectangle(cornerRadius: 6)
                    .fill(isSelected ? Color.accentColor.opacity(0.15) : Color.primary.opacity(0.05))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 6)
                    .stroke(isSelected ? Color.accentColor : Color.clear, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
    }
}

// MARK: - Add Step Button

struct AddStepButton: View {
    let onAdd: () -> Void

    var body: some View {
        Button {
            onAdd()
        } label: {
            Image(systemName: "plus")
                .font(.caption)
                .foregroundStyle(.secondary)
                .padding(8)
                .background(Circle().fill(Color.primary.opacity(0.05)))
        }
        .buttonStyle(.plain)
        .help("Add step")
    }
}

// MARK: - Step Editor Sheet

struct StepEditorSheet: View {
    @Binding var step: PipelineStep
    let availableTasks: [PromptCard]
    let availablePipelines: [PipelineDef]
    let onDelete: () -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var stepType: StepType = .task
    @State private var taskName: String = ""
    @State private var pipelineName: String = ""
    @State private var model: String = ""
    @State private var voice: String = ""
    @State private var contextText: String = ""

    enum StepType: String, CaseIterable {
        case task = "Task"
        case pipeline = "Pipeline"
    }

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Text("Edit Step")
                    .font(.headline)
                Spacer()
                Button("Done") {
                    saveAndDismiss()
                }
                .keyboardShortcut(.return)
            }
            .padding()

            Divider()

            Form {
                // Step type
                Picker("Type", selection: $stepType) {
                    ForEach(StepType.allCases, id: \.self) { type in
                        Text(type.rawValue).tag(type)
                    }
                }
                .pickerStyle(.segmented)

                if stepType == .task {
                    // Task selector
                    Picker("Task", selection: $taskName) {
                        Text("Select...").tag("")
                        ForEach(availableTasks) { task in
                            Text(task.name).tag(task.name)
                        }
                    }
                } else {
                    // Pipeline selector
                    Picker("Pipeline", selection: $pipelineName) {
                        Text("Select...").tag("")
                        ForEach(availablePipelines) { p in
                            Text(p.name).tag(p.name)
                        }
                    }
                }

                Section("Config Overrides") {
                    TextField("Model (e.g., claude:opus)", text: $model)
                    TextField("Voice", text: $voice)
                    TextField("Context paths (comma-separated)", text: $contextText)
                        .font(.system(size: 12, design: .monospaced))
                }

                Section {
                    Button("Delete Step", role: .destructive) {
                        onDelete()
                    }
                }
            }
            .formStyle(.grouped)
        }
        .frame(width: 400, height: 400)
        .onAppear {
            loadFromStep()
        }
    }

    private func loadFromStep() {
        if let task = step.task {
            stepType = .task
            taskName = task
        } else if let pipeline = step.pipeline {
            stepType = .pipeline
            pipelineName = pipeline
        }

        model = step.config?.model ?? ""
        voice = step.config?.voice ?? ""
        contextText = step.config?.context?.joined(separator: ", ") ?? ""
    }

    private func saveAndDismiss() {
        // Update step
        if stepType == .task {
            step.task = taskName.isEmpty ? nil : taskName
            step.pipeline = nil
        } else {
            step.task = nil
            step.pipeline = pipelineName.isEmpty ? nil : pipelineName
        }

        // Update config
        let contextPaths = contextText.split(separator: ",")
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }

        if model.isEmpty && voice.isEmpty && contextPaths.isEmpty {
            step.config = nil
        } else {
            step.config = StepConfig(
                model: model.isEmpty ? nil : model,
                voice: voice.isEmpty ? nil : voice,
                context: contextPaths.isEmpty ? nil : contextPaths
            )
        }

        dismiss()
    }
}

// MARK: - Pipeline Row (for sidebar)

struct PipelineRow: View {
    let pipeline: PipelineDef
    let isSelected: Bool
    let onSelect: () -> Void

    @State private var isHovering = false

    var body: some View {
        Button {
            onSelect()
        } label: {
            HStack(spacing: 8) {
                // Step dots visualization
                HStack(spacing: 2) {
                    ForEach(0..<min(pipeline.stepCount, 5), id: \.self) { _ in
                        Circle()
                            .fill(Color.accentColor)
                            .frame(width: 4, height: 4)
                    }
                    if pipeline.stepCount > 5 {
                        Text("+\(pipeline.stepCount - 5)")
                            .font(.system(size: 8))
                            .foregroundStyle(.secondary)
                    }
                }

                Text(pipeline.name)
                    .fontWeight(.medium)

                Spacer()

                Text("\(pipeline.stepCount)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(isSelected ? Color.accentColor.opacity(0.15) : (isHovering ? Color.primary.opacity(0.05) : Color.clear))
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovering in
            isHovering = hovering
        }
    }
}

// MARK: - New Pipeline Sheet

struct NewPipelineSheet: View {
    @Binding var isPresented: Bool
    let onCreate: (String) -> Void

    @State private var name = ""

    var body: some View {
        VStack(spacing: 20) {
            Text("New Pipeline")
                .font(.headline)

            TextField("Pipeline name", text: $name)
                .textFieldStyle(.roundedBorder)

            HStack {
                Button("Cancel") {
                    isPresented = false
                }
                .keyboardShortcut(.escape)

                Spacer()

                Button("Create") {
                    onCreate(name)
                    isPresented = false
                }
                .keyboardShortcut(.return)
                .disabled(name.isEmpty)
            }
        }
        .padding(24)
        .frame(width: 300)
    }
}

#Preview {
    @Previewable @State var pipeline = PipelineDef(
        name: "ship",
        steps: [
            PipelineStep(task: "design"),
            PipelineStep(task: "implement", config: StepConfig(model: "claude:opus", voice: "architect")),
            PipelineStep(task: "review"),
            PipelineStep(task: "polish"),
        ]
    )

    PipelineEditor(
        pipeline: $pipeline,
        availableTasks: [
            PromptCard(name: "design", content: "", defaultMode: .auto),
            PromptCard(name: "implement", content: "", defaultMode: .auto),
            PromptCard(name: "review", content: "", defaultMode: .auto),
            PromptCard(name: "polish", content: "", defaultMode: .auto),
        ],
        availablePipelines: [],
        onSave: {},
        onRun: {},
        onDelete: {}
    )
    .frame(width: 600, height: 200)
}

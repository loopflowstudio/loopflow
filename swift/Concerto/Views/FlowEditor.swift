// Visual editor for flow definitions.

import SwiftUI
import LoopflowCore

struct FlowEditor: View {
    @Binding var flow: FlowDef
    let availablePrompts: [PromptCard]
    let availableFlows: [FlowDef]
    let onSave: () -> Void
    let onRun: () -> Void
    let onDelete: () -> Void

    @State private var selectedStepIndex: Int?
    @State private var showingStepEditor = false

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            // Flow name
            HStack {
                TextField("Flow name", text: $flow.name)
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
                .help("Delete flow")
            }

            // Steps flow (horizontal scroll)
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(Array(flow.steps.enumerated()), id: \.element.id) { index, step in
                        StepChip(
                            step: step,
                            isSelected: selectedStepIndex == index
                        ) {
                            selectedStepIndex = index
                            showingStepEditor = true
                        }

                        if index < flow.steps.count - 1 {
                            Image(systemName: "arrow.right")
                                .font(.caption)
                                .foregroundStyle(.tertiary)
                        }
                    }

                    AddStepButton {
                        let newStep = Step(prompt: availablePrompts.first?.name ?? "step")
                        flow.steps.append(newStep)
                        selectedStepIndex = flow.steps.count - 1
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
                .disabled(flow.steps.isEmpty)
            }
        }
        .padding()
        .sheet(isPresented: $showingStepEditor) {
            if let index = selectedStepIndex, index < flow.steps.count {
                StepEditorSheet(
                    step: $flow.steps[index],
                    availablePrompts: availablePrompts,
                    availableFlows: availableFlows,
                    onDelete: {
                        flow.steps.remove(at: index)
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
    let step: Step
    let isSelected: Bool
    let onTap: () -> Void

    var body: some View {
        Button {
            onTap()
        } label: {
            HStack(spacing: 4) {
                Text(step.prompt)
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
    @Binding var step: Step
    let availablePrompts: [PromptCard]
    let availableFlows: [FlowDef]
    let onDelete: () -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var promptName: String = ""
    @State private var model: String = ""
    @State private var voice: String = ""
    @State private var contextText: String = ""

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
                // Prompt selector
                Picker("Prompt", selection: $promptName) {
                    Text("Select...").tag("")
                    ForEach(availablePrompts) { prompt in
                        Text(prompt.name).tag(prompt.name)
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
        .frame(width: 400, height: 350)
        .onAppear {
            loadFromStep()
        }
    }

    private func loadFromStep() {
        promptName = step.prompt

        model = step.config?.model ?? ""
        voice = step.config?.voice ?? ""
        contextText = step.config?.context?.joined(separator: ", ") ?? ""
    }

    private func saveAndDismiss() {
        // Update step
        step.prompt = promptName.isEmpty ? "step" : promptName

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

// MARK: - Flow Row (for sidebar)

struct FlowRow: View {
    let flow: FlowDef
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
                    ForEach(0..<min(flow.stepCount, 5), id: \.self) { _ in
                        Circle()
                            .fill(Color.white.opacity(0.7))
                            .frame(width: 4, height: 4)
                    }
                    if flow.stepCount > 5 {
                        Text("+\(flow.stepCount - 5)")
                            .font(.system(size: 8))
                            .foregroundStyle(.white.opacity(0.5))
                    }
                }

                Text(flow.name)
                    .fontWeight(.medium)
                    .foregroundStyle(.white)

                Spacer()

                Text("\(flow.stepCount)")
                    .font(.caption)
                    .foregroundStyle(.white.opacity(0.6))
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(isSelected ? Color.white.opacity(0.2) : (isHovering ? Color.white.opacity(0.1) : Color.clear))
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovering in
            isHovering = hovering
        }
    }
}

// MARK: - New Flow Sheet

struct NewFlowSheet: View {
    @Binding var isPresented: Bool
    let onCreate: (String) -> Void

    @State private var name = ""

    var body: some View {
        VStack(spacing: 20) {
            Text("New Flow")
                .font(.headline)

            TextField("Flow name", text: $name)
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
    @Previewable @State var flow = FlowDef(
        name: "ship",
        steps: [
            Step(prompt: "design"),
            Step(prompt: "implement", config: StepConfig(model: "claude:opus", voice: "architect")),
            Step(prompt: "review"),
            Step(prompt: "polish"),
        ]
    )

    FlowEditor(
        flow: $flow,
        availablePrompts: [
            PromptCard(name: "design", content: "", defaultMode: .auto),
            PromptCard(name: "implement", content: "", defaultMode: .auto),
            PromptCard(name: "review", content: "", defaultMode: .auto),
            PromptCard(name: "polish", content: "", defaultMode: .auto),
        ],
        availableFlows: [],
        onSave: {},
        onRun: {},
        onDelete: {}
    )
    .frame(width: 600, height: 200)
}

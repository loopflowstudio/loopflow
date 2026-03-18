// FlowTypeahead - unified flow/step picker with typeahead and candidate list.
// Single-select. Flows (multi-step) shown first, then steps.

import SwiftUI
import LoopflowCore

struct FlowTypeahead: View {
    enum Style: Equatable {
        case form
        case compact
    }

    let wave: WaveViewModel
    var initialSelection: String? = nil
    var style: Style = .form
    var onSubmitSelection: ((String) -> Void)? = nil
    let onSelect: (String) -> Void

    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    @State private var inputText = ""
    @State private var selectedFlow: String = ""
    @State private var isFieldFocused = false

    private var flows: [Flow] {
        repoState.flows.filter { $0.type == .flow }
    }

    private var steps: [Flow] {
        repoState.flows.filter { $0.type == .step }
    }

    /// All valid names: flows first, then steps.
    private var allNames: [String] {
        flows.map(\.name) + steps.map(\.name)
    }

    private var filteredFlows: [Flow] {
        if inputText.isEmpty { return flows }
        return flows.filter { $0.name.lowercased().contains(inputText.lowercased()) }
    }

    private var filteredSteps: [Flow] {
        if inputText.isEmpty { return steps }
        return steps.filter { $0.name.lowercased().contains(inputText.lowercased()) }
    }

    private var compactSuggestions: [Flow] {
        guard !inputText.isEmpty else { return [] }
        return Array((filteredFlows + filteredSteps).prefix(8))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            if style == .form {
                Text("Flow")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }

            // Selected flow chip + input
            HStack(spacing: 4) {
                if !selectedFlow.isEmpty {
                    flowChip(selectedFlow)
                }

                GhostTextField(
                    text: $inputText,
                    ghost: computeGhostCompletion(for: inputText),
                    placeholder: selectedFlow.isEmpty ? "Type flow name, tab to complete" : "",
                    onTab: acceptGhost,
                    onEnter: commitToken,
                    onBackspace: removeSelection,
                    onFocusChange: { isFieldFocused = $0 }
                )
                .frame(minWidth: 80, maxWidth: .infinity)
                .frame(height: 20)
            }
            .padding(style == .compact ? Spacing.md : Spacing.sm)
            .background(palette.surface)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))

            // Step chain for selected flow
            if style == .form,
               let flow = repoState.flows.first(where: { $0.name == selectedFlow }),
               flow.type == .flow, !flow.steps.isEmpty {
                let chain = flow.steps.map(\.prompt).joined(separator: " \u{2192} ")
                Text(chain)
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
                    .lineLimit(1)
            }

            // Candidate list when focused
            if isFieldFocused {
                candidateList
            }
        }
        .onAppear {
            selectedFlow = initialSelection ?? wave.flow
        }
        .onChange(of: initialSelection) { _, newValue in
            guard let newValue, !newValue.isEmpty else { return }
            selectedFlow = newValue
        }
    }

    // MARK: - Candidate List

    @ViewBuilder
    private var candidateList: some View {
        if style == .compact {
            if !compactSuggestions.isEmpty {
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    ForEach(compactSuggestions) { item in
                        Button {
                            selectFlow(item.name)
                        } label: {
                            HStack(spacing: Spacing.sm) {
                                Image(systemName: item.type == .flow ? "arrow.triangle.branch" : "square")
                                    .font(Typography.caption(10))
                                    .foregroundStyle(palette.textSecondary)
                                Text(item.name)
                                    .font(Typography.body(12))
                                    .foregroundStyle(palette.text)
                                Spacer()
                            }
                            .padding(.horizontal, Spacing.sm)
                            .padding(.vertical, Spacing.xs)
                            .background(palette.surface)
                            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
                        }
                        .buttonStyle(.plain)
                    }
                }
                .frame(maxWidth: 320, alignment: .leading)
            }
        } else {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                if !filteredFlows.isEmpty {
                    flowSection("Flows", items: filteredFlows, icon: "arrow.triangle.branch")
                }
                if !filteredSteps.isEmpty {
                    flowSection("Steps", items: filteredSteps, icon: "square")
                }
            }
        }
    }

    private func flowSection(_ title: String, items: [Flow], icon: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text(title)
                .font(Typography.caption(10))
                .foregroundStyle(palette.textSecondary)

            WrappingHStack(spacing: Spacing.xs) {
                ForEach(items) { item in
                    Button {
                        selectFlow(item.name)
                    } label: {
                        HStack(spacing: 4) {
                            Image(systemName: icon)
                                .font(Typography.caption(10))
                                .foregroundStyle(palette.textSecondary)
                            Text(item.name)
                                .font(Typography.code(12))
                        }
                        .padding(.horizontal, 8)
                        .padding(.vertical, 4)
                        .background(item.name == selectedFlow ? palette.accent.opacity(0.2) : palette.surface)
                        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }

    private func flowChip(_ name: String) -> some View {
        let isFlow = flows.contains(where: { $0.name == name })
        return TypeaheadChip(
            icon: isFlow ? "arrow.triangle.branch" : "square",
            label: name,
            helpText: name
        ) {
            selectedFlow = ""
            onSelect("")
        }
    }

    // MARK: - Completion Logic

    private func computeGhostCompletion(for text: String) -> String {
        if text.isEmpty { return "" }
        for name in allNames {
            if name.lowercased().hasPrefix(text.lowercased()) && name != text {
                return String(name.dropFirst(text.count))
            }
        }
        return ""
    }

    // MARK: - Actions

    private func selectFlow(_ name: String) {
        selectedFlow = name
        inputText = ""
        onSelect(name)
    }

    private func acceptGhost() {
        let ghost = computeGhostCompletion(for: inputText)
        guard !ghost.isEmpty else { return }
        inputText += ghost
        if allNames.contains(inputText) {
            selectFlow(inputText)
        }
    }

    private func commitToken() {
        let name = inputText.trimmingCharacters(in: .whitespacesAndNewlines)
        if !name.isEmpty {
            selectFlow(name)
            onSubmitSelection?(name)
            return
        }

        let existingSelection = selectedFlow.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !existingSelection.isEmpty else { return }
        onSubmitSelection?(existingSelection)
    }

    private func removeSelection() {
        guard !selectedFlow.isEmpty else { return }
        selectedFlow = ""
        onSelect("")
    }
}

#Preview {
    let repoState = RepoState()
    repoState.currentRepo = URL(fileURLWithPath: "/Users/jack/src/loopflow")
    repoState.flows = [
        Flow(name: "build", steps: [
            Step(prompt: "implement"),
            Step(prompt: "compress"),
            Step(prompt: "gate"),
            Step(prompt: "update-wave")
        ], type: .flow),
        Flow(name: "ship", steps: [
            Step(prompt: "design"),
            Step(prompt: "build"),
            Step(prompt: "review")
        ], type: .flow),
        Flow(name: "design-and-ship", steps: [
            Step(prompt: "design"),
            Step(prompt: "implement"),
            Step(prompt: "reduce"),
            Step(prompt: "polish")
        ], type: .flow),
        Flow(name: "grind", steps: [
            Step(prompt: "research"),
            Step(prompt: "iterate"),
            Step(prompt: "build"),
            Step(prompt: "gate")
        ], type: .flow),
        Flow(name: "design", steps: [Step(prompt: "design")], type: .step),
        Flow(name: "review", steps: [Step(prompt: "review")], type: .step),
        Flow(name: "implement", steps: [Step(prompt: "implement")], type: .step),
        Flow(name: "debug", steps: [Step(prompt: "debug")], type: .step),
    ]

    let wave = WaveViewModel(
        api: Wave(
            id: "test",
            name: "test-wave",
            repo: "/Users/jack/src/loopflow",
            flow: "build",
            direction: ["clarity"],
            area: ["swift"]
        ),
        recentSteps: []
    )

    return FlowTypeahead(wave: wave) { flow in
        print("Selected: \(flow)")
    }
    .environment(repoState)
    .frame(width: 500)
    .padding()
}

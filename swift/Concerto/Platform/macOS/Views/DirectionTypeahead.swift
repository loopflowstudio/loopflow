// DirectionTypeahead - fish-style typeahead for wave directions with chips.
// Shows a filtered candidate list below the input for discoverability.

import SwiftUI
import LoopflowCore

struct DirectionTypeahead: View {
    let wave: WaveViewModel
    let onSelect: ([String]) -> Void

    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    @State private var inputText = ""
    @State private var selectedDirections: [String] = []
    @State private var isFieldFocused = false

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Direction")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            HStack(spacing: 4) {
                ForEach(selectedDirections, id: \.self) { direction in
                    TypeaheadChip(
                        icon: "target",
                        label: direction.replacingOccurrences(of: "-", with: " ").capitalized,
                        helpText: direction
                    ) {
                        selectedDirections.removeAll { $0 == direction }
                        commitDirections()
                    }
                }

                GhostTextField(
                    text: $inputText,
                    ghost: computeGhostCompletion(for: inputText),
                    placeholder: selectedDirections.isEmpty ? "Type direction, tab to complete" : "",
                    onTab: acceptGhost,
                    onEnter: commitToken,
                    onBackspace: removeLastToken,
                    onFocusChange: { isFieldFocused = $0 }
                )
                .frame(minWidth: 80, maxWidth: .infinity)
                .frame(height: 20)
            }
            .padding(Spacing.sm)
            .background(palette.surface)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))

            // Candidate list — shown when focused and there are options
            if isFieldFocused, !filteredCandidates.isEmpty {
                candidateList
            }
        }
        .onAppear {
            selectedDirections = wave.direction
        }
    }

    // MARK: - Candidate List

    private var candidateList: some View {
        WrappingHStack(spacing: Spacing.xs) {
            ForEach(filteredCandidates, id: \.self) { candidate in
                Button {
                    selectCandidate(candidate)
                } label: {
                    Text(candidate)
                        .font(Typography.code(12))
                        .padding(.horizontal, 8)
                        .padding(.vertical, 4)
                        .background(palette.surface)
                        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
                }
                .buttonStyle(.plain)
            }
        }
    }

    // MARK: - Completion Logic

    private var candidates: [String] {
        repoState.availableDirections.filter { !selectedDirections.contains($0) }
    }

    private var filteredCandidates: [String] {
        if inputText.isEmpty { return candidates }
        return candidates.filter { $0.lowercased().contains(inputText.lowercased()) }
    }

    private func computeGhostCompletion(for text: String) -> String {
        if text.isEmpty { return "" }

        for candidate in candidates {
            if candidate.lowercased().hasPrefix(text.lowercased()) && candidate != text {
                return String(candidate.dropFirst(text.count))
            }
        }

        return ""
    }

    // MARK: - Actions

    private func selectCandidate(_ name: String) {
        if !selectedDirections.contains(name) {
            selectedDirections.append(name)
            commitDirections()
        }
        inputText = ""
    }

    private func acceptGhost() {
        let ghost = computeGhostCompletion(for: inputText)
        guard !ghost.isEmpty else { return }
        inputText += ghost
        // If the completed text is an exact candidate, commit it as a chip
        if candidates.contains(inputText) {
            commitToken()
        }
    }

    private func commitToken() {
        let name = inputText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty else { return }

        if !selectedDirections.contains(name) {
            selectedDirections.append(name)
            commitDirections()
        }
        inputText = ""
    }

    private func removeLastToken() {
        guard !selectedDirections.isEmpty else { return }
        selectedDirections.removeLast()
        commitDirections()
    }

    private func commitDirections() {
        onSelect(selectedDirections)
    }
}

#Preview {
    let repoState = RepoState()
    repoState.currentRepo = URL(fileURLWithPath: "/Users/jack/src/loopflow")
    repoState.availableDirections = ["product-engineer", "designer", "infra-engineer", "ceo", "conductor", "improviser", "listener", "craft", "flow", "scale"]

    let wave = WaveViewModel(
        api: Wave(
            id: "test",
            name: "test-wave",
            repo: "/Users/jack/src/loopflow",
            flow: "design",
            direction: ["product-engineer"],
            area: []
        ),
        recentSteps: []
    )

    return DirectionTypeahead(wave: wave) { directions in
        print("Selected: \(directions)")
    }
    .environment(repoState)
    .frame(width: 400)
    .padding()
}


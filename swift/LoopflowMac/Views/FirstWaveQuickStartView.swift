#if os(macOS)
import Loopflow
import SwiftUI

func availableWaveBootstrapRoles(existingWaveNames: Set<String>) -> [WaveBootstrapRole] {
    WaveBootstrapRole.allCases.filter { !existingWaveNames.contains($0.rawValue) }
}

struct FirstWaveQuickStartView: View {
    let repositoryName: String
    var isFirstWave = true
    var existingWaveNames: Set<String> = []
    let onStart: (WaveBootstrapChoice) async throws -> Void

    @Environment(\.palette) private var palette
    @State private var isNamingCustomWave = false
    @State private var customName = ""
    @State private var customTeamKey = ""
    @State private var startingChoice: WaveBootstrapChoice?
    @State private var errorMessage: String?
    @FocusState private var isCustomNameFocused: Bool

    private var trimmedCustomName: String {
        customName.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var availableRoles: [WaveBootstrapRole] {
        availableWaveBootstrapRoles(existingWaveNames: existingWaveNames)
    }

    private var customNameAlreadyExists: Bool {
        existingWaveNames.contains(trimmedCustomName.lowercased())
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.xl) {
                prompt

                VStack(spacing: Spacing.sm) {
                    ForEach(availableRoles) { role in
                        roleButton(role)
                    }
                }

                customChoice

                if let errorMessage {
                    Text(errorMessage)
                        .font(Typography.caption())
                        .foregroundStyle(Color.statusError)
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityIdentifier("first-wave-error")
                }
            }
            .frame(maxWidth: 560, alignment: .leading)
            .padding(.horizontal, Spacing.xxxl)
            .padding(.vertical, 56)
            .frame(maxWidth: .infinity, alignment: .center)
        }
        .background(palette.background)
        .accessibilityIdentifier("first-wave-quick-start")
    }

    private var prompt: some View {
        HStack(alignment: .top, spacing: Spacing.md) {
            Circle()
                .fill(palette.text)
                .frame(width: 9, height: 9)
                .padding(.top, 8)

            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text(isFirstWave ? "What kind of work are we starting?" : "Start a Wave")
                    .font(Typography.sectionTitle(26))
                    .foregroundStyle(palette.text)
                Text("Start \(repositoryName) with a familiar archetype, or name the Wave this product actually needs.")
                    .font(Typography.body())
                    .foregroundStyle(palette.textSecondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private func roleButton(_ role: WaveBootstrapRole) -> some View {
        Button {
            start(.role(role))
        } label: {
            HStack(spacing: Spacing.md) {
                VStack(alignment: .leading, spacing: Spacing.xxs) {
                    Text("\(role.title) · \(role.teamKey)")
                        .font(Typography.body())
                        .foregroundStyle(palette.text)
                    Text(role.summary)
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)
                        .lineLimit(2)
                }
                Spacer(minLength: Spacing.md)
                if startingChoice == .role(role) {
                    ProgressView()
                        .controlSize(.small)
                } else {
                    Image(systemName: "arrow.right")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(palette.textSecondary)
                }
            }
            .padding(.horizontal, Spacing.lg)
            .padding(.vertical, Spacing.md)
            .frame(maxWidth: .infinity, minHeight: HitTarget.touch, alignment: .leading)
            .background(palette.surface)
            .overlay {
                RoundedRectangle(cornerRadius: CornerRadius.md)
                    .stroke(palette.border, lineWidth: 1)
            }
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(startingChoice != nil)
        .accessibilityIdentifier("first-wave-role-\(role.rawValue)")
    }

    @ViewBuilder
    private var customChoice: some View {
        if isNamingCustomWave {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    Text("Wave name")
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)
                    TextField("Wave name", text: $customName)
                        .textFieldStyle(.plain)
                        .font(Typography.body())
                        .foregroundStyle(palette.text)
                        .padding(.horizontal, Spacing.md)
                        .frame(height: HitTarget.touch)
                        .background(palette.surfaceMuted)
                        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                        .focused($isCustomNameFocused)
                        .onChange(of: customName) { oldName, newName in
                            let oldSuggestion = suggestedWaveTeamKey(for: oldName)
                            if customTeamKey.isEmpty || customTeamKey == oldSuggestion {
                                customTeamKey = suggestedWaveTeamKey(for: newName)
                            }
                        }
                        .disabled(startingChoice != nil)
                        .accessibilityIdentifier("first-wave-custom-name")
                    if customNameAlreadyExists {
                        Text("That Wave already exists.")
                            .font(Typography.caption(11))
                            .foregroundStyle(Color.statusError)
                    }
                }

                VStack(alignment: .leading, spacing: Spacing.xs) {
                    Text("Task tag")
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)
                    TextField("ABC", text: $customTeamKey)
                        .textFieldStyle(.plain)
                        .font(Typography.code())
                        .foregroundStyle(palette.text)
                        .textCase(.uppercase)
                        .padding(.horizontal, Spacing.md)
                        .frame(width: 96, height: HitTarget.touch)
                        .background(palette.surfaceMuted)
                        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                        .onChange(of: customTeamKey) { _, value in
                            let normalized = normalizedWaveTeamKey(value)
                            if normalized != value {
                                customTeamKey = normalized
                            }
                        }
                        .disabled(startingChoice != nil)
                        .accessibilityIdentifier("first-wave-custom-tag")
                    Text("This durable tag prefixes every Task and is costly to change.")
                        .font(Typography.caption(11))
                        .foregroundStyle(palette.textSecondary)
                }

                HStack {
                    Spacer()
                    Button {
                        startCustomWave()
                    } label: {
                        if case .custom = startingChoice {
                            ProgressView()
                                .controlSize(.small)
                        } else {
                            Text(isValidWaveTeamKey(customTeamKey) ? "Start · \(customTeamKey)" : "Start")
                        }
                    }
                    .buttonStyle(DarkButtonStyle())
                    .disabled(
                        startingChoice != nil
                            || trimmedCustomName.isEmpty
                            || customNameAlreadyExists
                            || !isValidWaveTeamKey(customTeamKey)
                    )
                    .accessibilityIdentifier("first-wave-custom-submit")
                }
            }
        } else {
            Button {
                isNamingCustomWave = true
                Task { @MainActor in isCustomNameFocused = true }
            } label: {
                Label("Name a Wave", systemImage: "plus")
                    .font(Typography.body())
            }
            .buttonStyle(GhostButtonStyle())
            .disabled(startingChoice != nil)
            .accessibilityIdentifier("first-wave-custom")
        }
    }

    private func startCustomWave() {
        guard !trimmedCustomName.isEmpty,
              isValidWaveTeamKey(customTeamKey)
        else { return }
        start(.custom(
            name: trimmedCustomName,
            teamKey: customTeamKey,
            teamName: trimmedCustomName
        ))
    }

    private func start(_ choice: WaveBootstrapChoice) {
        guard startingChoice == nil else { return }
        startingChoice = choice
        errorMessage = nil
        Task {
            defer { startingChoice = nil }
            do {
                try await onStart(choice)
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }
}

private extension WaveBootstrapRole {
    var summary: String {
        switch self {
        case .product:
            "Decide what to build and prove it works for users."
        case .infrastructure:
            "Keep development, delivery, and runtime reliable."
        case .intelligence:
            "Turn what the system learns into sharper decisions."
        case .operations:
            "Keep recurring work legible and moving."
        }
    }
}
#endif

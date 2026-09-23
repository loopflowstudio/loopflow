#if os(macOS)
import Loopflow
import SwiftUI

/// Podium-bar entry point to this repository's Sessions.
struct SessionsButton: View {
    @Bindable var model: PodiumModel
    let onOpen: () -> Void

    private var count: Int? {
        guard model.repoPath != nil, model.sessions.errorMessage == nil else { return nil }
        return model.sessions.value?.count
    }

    private var summary: String {
        if model.repoPath == nil { return "Select a repository for Sessions" }
        if let error = model.sessions.errorMessage { return "Sessions unavailable: \(error)" }
        guard let count else { return "Reading Sessions…" }
        return count == 0 ? "No Sessions" : "\(count) Sessions"
    }

    var body: some View {
        Button {
            onOpen()
        } label: {
            Label(count.map(String.init) ?? "?", systemImage: "terminal")
                .font(Typography.caption(9).weight(.semibold))
                .foregroundStyle(.white)
                .padding(.horizontal, Spacing.sm)
                .frame(height: HitTarget.comfortable)
                .background(Color.white.opacity(count == 0 ? 0.06 : 0.16), in: Capsule())
        }
        .buttonStyle(.plain)
        .help(summary)
        .accessibilityLabel("Sessions")
        .accessibilityValue(summary)
        .accessibilityIdentifier("podium-sessions")
    }
}
#endif

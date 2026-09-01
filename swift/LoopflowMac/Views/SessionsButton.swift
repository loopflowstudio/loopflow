#if os(macOS)
import Loopflow
import SwiftUI

/// Podium-bar entry point to this repository's Sessions.
struct SessionsButton: View {
    @Bindable var model: PodiumModel
    let onOpen: () -> Void

    private var count: Int {
        (model.sessions.value ?? []).count
    }

    var body: some View {
        Button {
            onOpen()
        } label: {
            Label("\(count)", systemImage: "terminal")
                .font(Typography.caption(9).weight(.semibold))
                .foregroundStyle(.white)
                .padding(.horizontal, Spacing.sm)
                .frame(height: HitTarget.comfortable)
                .background(Color.white.opacity(count == 0 ? 0.06 : 0.16), in: Capsule())
        }
        .buttonStyle(.plain)
        .help(count == 0 ? "No Sessions" : "\(count) Sessions")
        .accessibilityLabel("Sessions")
        .accessibilityValue(count == 0 ? "No Sessions" : "\(count) Sessions")
        .accessibilityIdentifier("podium-sessions")
    }
}
#endif

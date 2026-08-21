#if os(macOS)
import Loopflow
import SwiftUI

/// Podium-bar entry point to the repo-scoped Sessions surface. Shows the count
/// of User-targeted Asks; tapping returns to the native multiplexer.
struct UserAskAttentionButton: View {
    @Bindable var model: PodiumModel
    let onOpen: () -> Void

    private var count: Int {
        (model.userAskAttention.value ?? []).count
    }

    var body: some View {
        Button {
            onOpen()
        } label: {
            Label("\(count)", systemImage: "person.crop.circle.badge.questionmark")
                .font(Typography.caption(9).weight(.semibold))
                .foregroundStyle(.white)
                .padding(.horizontal, Spacing.sm)
                .frame(height: HitTarget.comfortable)
                .background(Color.white.opacity(count == 0 ? 0.06 : 0.16), in: Capsule())
        }
        .buttonStyle(.plain)
        .help(count == 0 ? "No requested sessions" : "\(count) requested sessions")
        .accessibilityLabel("Requested sessions")
        .accessibilityValue(count == 0 ? "No requested sessions" : "\(count) requested sessions")
        .accessibilityIdentifier("podium-user-attention")
    }
}
#endif

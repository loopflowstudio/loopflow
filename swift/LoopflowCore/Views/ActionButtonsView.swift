import SwiftUI

public struct ActionButtonsView: View {
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.palette) private var palette

    public let actions: [SuggestedAction]
    public let onTap: (SuggestedAction) -> Void

    public init(actions: [SuggestedAction], onTap: @escaping (SuggestedAction) -> Void) {
        self.actions = actions
        self.onTap = onTap
    }

    private var isCompact: Bool {
        horizontalSizeClass == .compact
    }

    public var body: some View {
        Group {
            if isCompact {
                VStack(alignment: .leading, spacing: Spacing.sm) {
                    ForEach(actions) { action in
                        actionButton(action, fullWidth: true)
                    }
                }
            } else {
                LazyVGrid(
                    columns: [GridItem(.adaptive(minimum: 180), spacing: Spacing.sm)],
                    alignment: .leading,
                    spacing: Spacing.sm
                ) {
                    ForEach(actions) { action in
                        actionButton(action, fullWidth: false)
                    }
                }
            }
        }
        .animation(DesignAnimation.standard(reduceMotion), value: actions)
    }

    @ViewBuilder
    private func actionButton(_ action: SuggestedAction, fullWidth: Bool) -> some View {
        Button {
            onTap(action)
        } label: {
            VStack(alignment: .leading, spacing: Spacing.xs) {
                Text(action.label)
                    .font(Typography.body())
                    .foregroundStyle(palette.text)
                    .multilineTextAlignment(.leading)

                if let description = action.description {
                    Text(description)
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)
                        .multilineTextAlignment(.leading)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, Spacing.md)
            .padding(.vertical, Spacing.sm)
            .frame(minHeight: HitTarget.touch, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: CornerRadius.md)
                    .fill(palette.surface)
            )
            .overlay(
                RoundedRectangle(cornerRadius: CornerRadius.md)
                    .stroke(palette.border, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
        .frame(maxWidth: fullWidth ? .infinity : nil, alignment: .leading)
        .accessibleButton(action.label, hint: action.description)
    }
}

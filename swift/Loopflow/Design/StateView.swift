import SwiftUI

/// Every empty and failed state: left-aligned, a serif title, then a sentence
/// and buttons where the state has them. No icon over a centred title.
public struct StateView<Actions: View>: View {
    let title: String
    let sentence: String?
    let actions: Actions
    @Environment(\.palette) private var palette

    public init(_ title: String, sentence: String? = nil, @ViewBuilder actions: () -> Actions) {
        self.title = title
        self.sentence = sentence
        self.actions = actions()
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            Text(verbatim: title)
                .serif(.title)
                .foregroundStyle(palette.text)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityAddTraits(.isHeader)
            if let sentence {
                Text(verbatim: sentence)
                    .font(Typography.text)
                    .foregroundStyle(palette.textSecondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            VStack(alignment: .leading, spacing: Spacing.sm) { actions }
                .padding(.top, Spacing.xs)
        }
        .padding(Spacing.xl)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

extension StateView where Actions == EmptyView {
    public init(_ title: String, sentence: String? = nil) {
        self.init(title, sentence: sentence) { EmptyView() }
    }
}

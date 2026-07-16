#if os(macOS)
import Loopflow
import SwiftUI

/// The Control destination: the deliberate, non-default surface for steering and
/// observing the machine. Chat stays the default Wave experience; Control is
/// reached from the Wave header. Its first view is Active Sessions; Run History
/// is named but deferred to a quiet, disabled affordance.
struct ControlView: View {
    let onClose: () -> Void

    @Environment(\.palette) private var palette
    @State private var section: ControlSection = .activeSessions

    enum ControlSection: String, CaseIterable {
        case activeSessions
        case runHistory

        var title: String {
            switch self {
            case .activeSessions: "Active Sessions"
            case .runHistory: "Run History"
            }
        }

        var available: Bool { self == .activeSessions }
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            switch section {
            case .activeSessions:
                ActiveSessionsView()
            case .runHistory:
                EmptyView()
            }
        }
        .frame(minWidth: 660, minHeight: 540)
        .background(palette.background)
    }

    private var header: some View {
        HStack(spacing: Spacing.md) {
            Text("Control")
                .font(Typography.sectionTitle())
                .foregroundStyle(palette.text)

            HStack(spacing: Spacing.xs) {
                ForEach(ControlSection.allCases, id: \.self) { tab in
                    sectionTab(tab)
                }
            }

            Spacer()

            Button(action: onClose) {
                Image(systemName: "xmark")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }
            .buttonStyle(.plain)
            .help("Close Control")
            .accessibilityLabel("Close Control")
        }
        .padding(.horizontal, Spacing.xl)
        .padding(.vertical, Spacing.md)
    }

    private func sectionTab(_ tab: ControlSection) -> some View {
        let selected = section == tab
        return Button {
            if tab.available { section = tab }
        } label: {
            HStack(spacing: Spacing.xs) {
                Text(tab.title)
                    .font(Typography.caption(12))
                if !tab.available {
                    Text("soon")
                        .font(Typography.caption(9))
                        .padding(.horizontal, Spacing.xs)
                        .padding(.vertical, 1)
                        .background(palette.surfaceMuted)
                        .clipShape(Capsule())
                }
            }
            .foregroundStyle(
                tab.available
                    ? (selected ? palette.text : palette.textSecondary)
                    : palette.textSecondary.opacity(0.6)
            )
            .padding(.horizontal, Spacing.sm)
            .padding(.vertical, Spacing.xxs)
            .background(selected ? palette.surfaceMuted : Color.clear)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
        }
        .buttonStyle(.plain)
        .disabled(!tab.available)
        .help(tab.available ? "" : "Run History is coming soon")
        .accessibilityLabel(tab.title)
        .accessibilityHint(tab.available ? "" : "Coming soon")
    }
}

#endif

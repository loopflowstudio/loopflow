#if os(macOS)
import SwiftUI
import LoopflowCore

/// The wave detail pane: a header (wave identity + close) over the live WaveChat
/// transcript. WaveChat is the sole detail surface — the wave runs in its own
/// `lf wave` process and Concerto observes it through its chat server rather than
/// hosting a raw terminal.
struct WaveDetailPane: View {
    let wave: WaveViewModel
    let repoPath: String
    let onClose: () -> Void

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            WaveChatView(repoPath: repoPath, waveName: wave.name)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private var header: some View {
        HStack(spacing: Spacing.sm) {
            Image(systemName: wave.statusIndicator.icon)
                .foregroundStyle(wave.statusIndicator.color)
            Text(wave.displayName)
                .font(Typography.sectionTitle())
                .foregroundStyle(palette.text)
            Text("WaveChat")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            Spacer()

            Button {
                onClose()
            } label: {
                Image(systemName: "xmark")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }
            .buttonStyle(.plain)
            .help("Close wave")
            .accessibilityLabel("Close wave")
        }
        .padding(.horizontal, Spacing.xl)
        .padding(.vertical, Spacing.md)
    }
}

#endif

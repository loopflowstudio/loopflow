#if os(macOS)
import SwiftUI
import LoopflowCore

struct DiagnosticsView: View {
    @Environment(\.palette) private var palette
    @Environment(\.dismiss) private var dismiss
    @State private var logText: String = ""

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            HStack {
                Text("Diagnostics")
                    .font(Typography.sectionTitle())
                    .foregroundStyle(palette.text)
                Spacer()
                Button("Refresh") {
                    loadLogs()
                }
                .buttonStyle(GhostButtonStyle())
                Button("Close") {
                    dismiss()
                }
                .buttonStyle(GhostButtonStyle())
            }

            Text("Log file: \(LoggingService.logPath())")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            TextEditor(text: $logText)
                .font(Typography.code())
                .frame(minHeight: 300)
                .background(palette.surfaceMuted)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        }
        .padding(Spacing.xl)
        .background(palette.background)
        .frame(minWidth: 640, minHeight: 420)
        .onAppear {
            loadLogs()
        }
    }

    private func loadLogs() {
        logText = LoggingService.read()
    }
}

#endif

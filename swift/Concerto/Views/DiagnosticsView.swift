import SwiftUI
import LoopflowCore

struct DiagnosticsView: View {
    @Environment(\.dismiss) private var dismiss
    @State private var logText: String = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Diagnostics")
                    .font(.headline)
                Spacer()
                Button("Refresh") {
                    loadLogs()
                }
                Button("Close") {
                    dismiss()
                }
            }

            Text("Log file: \(LoggingService.logPath())")
                .font(.caption)
                .foregroundStyle(.secondary)

            TextEditor(text: $logText)
                .font(.system(.body, design: .monospaced))
                .frame(minHeight: 300)
                .background(Color(.textBackgroundColor))
                .cornerRadius(8)
        }
        .padding()
        .frame(minWidth: 640, minHeight: 420)
        .onAppear {
            loadLogs()
        }
    }

    private func loadLogs() {
        logText = LoggingService.read()
    }
}

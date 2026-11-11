import Loopflow
import SwiftUI

/// Native receipts supply progress; a click only requests another capacity check.
struct SessionRecoveryView: View {
    let recovery: SessionRecovery
    let recover: () -> Void

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                Text(recovery.state.label).font(.caption.bold())
                Text(recovery.reason).font(.caption).foregroundStyle(.secondary)
                if let reset = recovery.retryAt {
                    Text("Capacity reset: \(Date(timeIntervalSince1970: Double(reset)), style: .relative)")
                        .font(.caption).foregroundStyle(.secondary)
                }
            }
            Spacer(minLength: 0)
            if recovery.canRetry {
                Button("Recover affected sessions", action: recover).font(.caption)
            }
        }
        .padding(10)
        .accessibilityElement(children: .contain)
    }
}

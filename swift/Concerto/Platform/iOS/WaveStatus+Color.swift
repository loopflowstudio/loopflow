#if os(iOS)
import SwiftUI
import LoopflowCore

extension WaveStatus {
    var mobileStatusColor: Color {
        switch self {
        case .idle: .statusNeutral
        case .running: .statusSuccess
        case .waiting: .statusWarning
        case .failed: .statusError
        case .paused: .statusInfo
        }
    }
}
#endif

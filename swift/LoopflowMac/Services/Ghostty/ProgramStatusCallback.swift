import Foundation
import Loopflow

/// Call synchronously while the Ghostty action's borrowed strings are alive.
/// The returned value contains only owned Swift strings and crosses actors safely.
func copyProgramStatusEvent(
    event: Int, state: Int, kind: Int, progress: Int,
    id: UnsafePointer<CChar>?, app: UnsafePointer<CChar>?,
    title: UnsafePointer<CChar>?, msg: UnsafePointer<CChar>?
) -> ProgramStatusEvent? {
    switch event {
    case 1: return .prompt
    case 2: return .reset
    case 3: return .exit
    case 0:
        let states: [ProgramStatusState] = [.idle, .working, .done, .blocked, .error, .clear]
        let kinds: [ProgramStatusKind] = [.permission, .question, .auth]
        guard states.indices.contains(state) else { return nil }
        let recordId = id.map { String(cString: $0) }
        return .report(ProgramStatusReport(
            state: states[state], id: recordId?.isEmpty == false ? recordId : nil,
            kind: kinds.indices.contains(kind) ? kinds[kind] : nil,
            progress: (0...100).contains(progress) ? UInt8(progress) : nil,
            app: app.map { String(cString: $0) }, title: title.map { String(cString: $0) },
            msg: msg.map { String(cString: $0) }
        ))
    default: return nil
    }
}

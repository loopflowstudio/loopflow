import Foundation
import Loopflow

extension Notice.Action {
    /// The notice follows the keyboard Undo stack while this edit is on top.
    @MainActor static func undo<T: AnyObject>(_ name: String, manager: UndoManager?, target: T,
                                              restore: @escaping @MainActor (T) -> Void) -> Self {
        manager?.registerUndo(withTarget: target, handler: restore)
        manager?.setActionName(name)
        return Self("Undo") { [weak manager] in
            if let manager, manager.undoActionName == name { manager.undo() }
            else { restore(target) }
        }
    }
}

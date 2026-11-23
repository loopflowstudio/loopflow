import Loopflow
import SwiftUI

struct GeneralSettings: View {
    let notices: NoticeCenter
    private let defaults: UserDefaults
    @AppStorage private var savedAutosave: Bool?
    private let undoManager: UndoManager?

    init(notices: NoticeCenter, undoManager: UndoManager?, defaults: UserDefaults = .standard) {
        self.notices = notices
        self.undoManager = undoManager
        self.defaults = defaults
        _savedAutosave = AppStorage(UserDefaults.taskFileAutosaveKey, store: defaults)
    }

    var body: some View {
        Form {
            Section("Task Files") {
                Toggle("Autosave Task Files", isOn: Binding(
                    get: { savedAutosave ?? true },
                    set: { saveAutosave($0) }
                ))
                .toggleStyle(.switch)
            }
        }
        .formStyle(.grouped)
    }

    private func saveAutosave(_ enabled: Bool) {
        let previous = savedAutosave
        savedAutosave = enabled
        let manager = undoManager
        let restore = { [weak notices] (defaults: UserDefaults) in
            guard defaults.object(forKey: UserDefaults.taskFileAutosaveKey) as? Bool == enabled else { return }
            defaults.set(previous, forKey: UserDefaults.taskFileAutosaveKey)
            notices?.clear(key: UserDefaults.taskFileAutosaveKey)
        }
        manager?.registerUndo(withTarget: defaults, handler: restore)
        manager?.setActionName("Autosave Task Files")
        notices.post(Notice(enabled ? "Task file autosave on" : "Task file autosave off",
                            key: UserDefaults.taskFileAutosaveKey, action: .init("Undo") { [weak manager, defaults] in
            if let manager, manager.undoActionName == "Autosave Task Files" {
                manager.undo()
            } else {
                restore(defaults)
            }
        }))
    }
}
